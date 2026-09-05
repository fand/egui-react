# プラン: async

タスク定義は [task.md](task.md)。設計の根拠は [docs/ARCHITECTURE.md](../../ARCHITECTURE.md)。本書は実装手順と確認方法を定める。実装中にここから外れる判断をした場合は本書を更新し、設計上の意味があれば ARCHITECTURE.md も更新する。

## 0. 全体

1 フェーズ(フェーズ 6)を 1 PR で進める。コミットは 2 つ以上に分ける(`use_future` / `Suspense` + example)。触るのは `react-egui`(hook 1 つ、`spawn`、`Store` のカウンタ)、`react-egui-elements`(`Suspense`)、`examples/fetch`、CI、docs。`Slot` / `Cx` / マクロ / app には手を入れない(入れる必要が出たら 9 章に書く)。

追加する依存(`[workspace.dependencies]` に pin する)。

| crate | 用途 | 場所 |
|---|---|---|
| pollster | native の executor(`block_on`) | react-egui(`cfg(not(target_arch = "wasm32"))`) |
| ehttp 0.7(feature `native-async`) | fetch example の HTTP クライアント(native = ureq、wasm = fetch API)。`fetch_async` は native では `native-async` が要る | examples/fetch |

`wasm-bindgen-futures` は既に workspace にあり、`react-egui` の wasm 依存に足す。

## 1. `use_future` と `spawn`(`crates/react-egui/src/future.rs`)

### 1.1 API

```rust
use std::task::Poll;

/// native では Send、wasm では非 Send の future を同じ名前で受ける。
#[cfg(not(target_arch = "wasm32"))]
pub trait SpawnFuture<T>: Future<Output = T> + Send + 'static {}
#[cfg(not(target_arch = "wasm32"))]
impl<T, F: Future<Output = T> + Send + 'static> SpawnFuture<T> for F {}

#[cfg(target_arch = "wasm32")]
pub trait SpawnFuture<T>: Future<Output = T> + 'static {}
#[cfg(target_arch = "wasm32")]
impl<T, F: Future<Output = T> + 'static> SpawnFuture<T> for F {}

#[track_caller]
pub fn use_future<'s, D: Hash, T: Send + 'static>(
    cx: &mut Cx<'s, '_>,
    deps: D,
    f: impl FnOnce() -> impl SpawnFuture<T>,   // 実装では generic F
) -> &'s Poll<T>

/// 起動して忘れる。結果は `Dispatch` で送る。
pub fn spawn(fut: impl SpawnFuture<()>)
```

- `f` は呼び出し位置でその場で呼ぶ(`use_effect` と同じ)。ローカルや `State` の guard を読んで future を組み立ててよい。future 自身は `'static`(`move` で clone した値を持つ)。
- `T: Send + 'static` は wasm でも要求する。bound を platform ごとに変えるのは future 側だけにして、ユーザーの型は 1 種類で済ませる。
- `spawn` は `task::spawn` をそのまま公開したもの。`use_future` が deps 駆動なのに対し、ハンドラから命令的に起動する用(`spawn(async move { dispatch.send(Msg::Saved(api.await)) })`)。
- `lib.rs` から `use_future` / `spawn` / `SpawnFuture` を re-export し、`prelude` に `use_future` / `spawn` と `std::task::Poll` を足す。

### 1.2 スロットの中身

スロットは 2 つ使う(`use_reducer` と同じ分け方)。

- **状態スロット**(`scope_id.with(location_key)`)。値は `()`。`deps_hash` に deps のハッシュ、`memo` の `FrozenVec` に `Poll<T>` を積む。`Pending` を起動時に 1 つ、届いたら `Ready(T)` を 1 つ。返り値は `memo_last()` の downcast。
- **受信スロット**(`id.with("__react_egui_future_inbox")`)。値は `Arc<Mutex<Option<(u64, T)>>>` と現在の世代 `Cell<u64>` をまとめた構造体。

```rust
struct Inbox<T> {
    cell: Arc<Mutex<Option<(u64, T)>>>,   // (世代, 結果)
    generation: Cell<u64>,                // 最後に起動した future の世代
}
```

### 1.3 訪問時の手順

1. 両スロットを `store.slot(..)` で取る(初回は `Inbox { cell: None, generation: 0 }`)。
2. `hash = deps_hash(&deps)`。`slot.deps_hash() != Some(hash)` なら**起動**:
   - `generation += 1`、`fut = f()`。
   - `memo_push(Poll::<T>::Pending)`、`set_deps_hash(hash)`。
   - `task::spawn(async move { let v = fut.await; inbox に書く; ctx.request_repaint(); })`。`cell` と `ctx`(`store.ctx().clone()`)は `move`。書き込みは「セルが空か、入っている世代より新しい時だけ上書き」(1.5)。
3. 起動しなかった場合は**受信**を試みる: `lock(&cell).take()` が `Some((gen, v))` で `gen == generation` なら `memo_push(Poll::Ready(v))`。世代が違えば捨てる。
4. `memo_last()` を `&Poll<T>` に downcast する。`Pending` なら `store.note_pending()`(2.1)。返す。

起動の直後は `Pending` を返すだけで受信を試みない(その場で完了していても次フレームで拾う。`request_repaint` が飛ぶので取りこぼさない)。同一フレームの 2 パス目は deps が一致するので起動せず、受信を試みるだけである。

### 1.4 `task::spawn`

```rust
mod task {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
        if let Err(err) = std::thread::Builder::new()
            .name("react-egui-future".into())
            .spawn(move || pollster::block_on(fut))
        {
            log::error!("react-egui: could not spawn a thread for use_future: {err}");
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn spawn(fut: impl Future<Output = ()> + 'static) {
        wasm_bindgen_futures::spawn_local(fut);
    }
}
```

スレッド生成に失敗したら future は捨てられ `Pending` のまま止まる。ログだけ出す(panic はしない)。

### 1.5 unmount と古い結果

- unmount でスロットが落ちると `Inbox` も落ちる。走っている future は自分の `Arc` を持っているので書き込みは成功し、`request_repaint` が 1 回飛ぶ。次のフレームで誰も読まず、`Arc` の最後の参照が future の終了と共に消える。
- deps 変更後に古い future が完了した場合、`cell` に古い世代が入る。訪問ごとに `take` して世代を見るので、受信手順で捨てられる。
- 訪問の前に「新しい方 → 古い方」の順で両方が届くと、古い方が新しい結果を上書きして失う。これを避けるため、future 側の書き込みは「セルが空か、入っている世代より新しい時だけ上書き」にする(`Some((old, _)) if old > gen` なら何もしない)。

### 1.6 `Cargo.toml`

```toml
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
pollster.workspace = true

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen-futures.workspace = true
```

## 2. `Suspense`

### 2.1 `Store` の suspense カウンタ(`store.rs`)

`contexts` のスタックと同じ置き方で、`suspense: RefCell<Vec<usize>>` を持つ。`begin_pass` で clear。

```rust
impl Store {
    /// 境界に入る。カウンタ 0 を積む。
    pub fn begin_suspense(&self);
    /// 境界を出る。積んだカウンタを返す(= 中で Pending だった use_future の数)。
    pub fn end_suspense(&self) -> usize;
    /// use_future が Pending を返す時に呼ぶ。最も近い境界のカウンタを +1。境界が無ければ何もしない。
    pub fn note_pending(&self);
}
```

`use_future` は返す直前、`Pending` なら `note_pending()` を呼ぶ(1.3 の手順 4)。起動直後も受信で `Pending` のままでも同じ。

### 2.2 要素(`crates/react-egui-elements/src/suspense.rs`)

```rust
/// 中の `use_future` が 1 つでも Pending なら children の代わりに fallback を描く。
#[component(shares_ui)]
pub fn Suspense(cx: &mut Cx, fallback: impl View, children: impl View) {
    let (store, scope) = (cx.store, cx.scope_id());
    let suspended = use_handle(cx, || true);          // 初期は suspended(task.md 決めごと)

    if suspended.get() {
        // 1. fallback を可視の surface に描く
        fallback.show(cx);
        // 2. children をオフスクリーンの不可視 Ui に描く(hooks を走らせる)
        let mut ui = egui::Ui::new(
            store.ctx().clone(),
            scope.with("suspense_offscreen"),
            egui::UiBuilder::new()
                .max_rect(OFFSCREEN_RECT)          // 画面外、十分に大きい固定矩形
                .invisible()
                .sizing_pass(),
        );
        store.begin_suspense();
        {
            let mut child = Cx::new(store, &mut ui, scope);
            children.show(&mut child);
        }
        let pending = store.end_suspense();
        // 3. 全部揃ったら可視に切り替える。同一フレームでやり直す
        if pending == 0 {
            suspended.set(false);
            store.ctx().request_discard("suspense resolved");
        }
    } else {
        store.begin_suspense();
        children.show(cx);
        let pending = store.end_suspense();
        if pending > 0 {
            suspended.set(true);
            store.ctx().request_discard("suspense pending");
        }
    }
}
```

- `shares_ui` にするのは、`Suspense` が自分の `Ui` / leaf を作らず、children と fallback を親の surface(Ui でも Taffy でも)にそのまま流すため。`<View>` の中に置けば children の `<View>` は親の taffy ツリーの子になる。
- children のスコープは可視でもオフスクリーンでも `scope`(`Suspense` 自身の `scope_id()`)。`Cx::new(store, &mut ui, scope)` の第 3 引数がそれ。hook のスロットが両経路で同じ Id になり、切り替えで state と future が保たれる。fallback も同じ `cx` に描くが、`rsx!` の要素 Id は行・列で分かれるので children と衝突しない。
- `Handle::set` は `request_repaint` するが、呼ぶのは切り替えの瞬間だけ(直後に `request_discard` もする)なので、suspended のまま毎フレーム repaint することはない。
- `request_discard` は egui の `max_passes`(ランナー既定 3)の範囲で同一フレーム内にやり直す。使い切って却下された場合はランナーが `request_repaint` するので次フレームで揃う。初期状態を suspended にしてあるので、却下されても見えるのは fallback。
- オフスクリーン `Ui` の矩形は `Rect::from_min_size(pos2(-1.0e5, -1.0e5), vec2(4096.0, 4096.0))` のような固定値。egui_taffy が毎パス同じ大きさを見るようにして、無駄な discard を避ける。
- suspended 中の children の `use_effect` は走る(React と違う)。rustdoc と ARCHITECTURE.md に書く。
- children のハンドラはオフスクリーンでは発火しない(`invisible()` が操作も無効にする)。
- `prelude` に `Suspense` を足す。

## 3. テスト

### 3.1 `crates/react-egui/tests/future.rs`

`tests/common` の `run_app` を使う。future の完了は `std::sync::mpsc` の `Receiver` を future の中で `recv()` してテスト側から `send` する(future は自分のスレッドにいるので blocking でよい)。完了は別スレッドなので、`ctx.has_requested_repaint()` が立つまで最大 2 秒 `sleep(10ms)` で待つ補助関数 `wait_for_repaint(&harness)` を書く。

| # | テスト | 確認すること |
|---|---|---|
| 6-1 | `pending_then_ready_after_repaint` | 初回 `run` は `Pending`(ラベル "loading")。`send` 後に repaint が要求され、次の `run` で `Ready` の値が表示される |
| 6-2 | `ready_reference_lives_next_to_a_state_guard` | `let mut n = use_state(..); let r = use_future(..);` の後で `*n += 1` と `r` の読みが同時に書ける(コンパイルが通り、値が正しい) |
| 6-3 | `deps_change_restarts_and_stale_result_is_dropped` | deps(`State<u32>`)を変えると `Pending` に戻り、future が 2 回起動する。古い方を後から完了させても表示は新しい方のまま。先に古い方、後に新しい方の順でも最終表示は新しい方 |
| 6-4 | `stale_result_does_not_overwrite_a_newer_one_before_visit` | 両方の future を完了させてから 1 回だけ `run`。新しい世代の値が出る(1.5 の上書き条件) |
| 6-5 | `a_two_pass_frame_spawns_once` | 1 パス目で `request_discard` して 2 パス走らせる。`f` の呼び出し回数(`Rc<Cell<usize>>`)が 1 |
| 6-6 | `unmount_while_pending_does_not_panic_and_frees_the_slot` | `use_future` を持つ子を `if` で外す。`store.len()` が外す前の水準に戻る。その後 `send` して完了させても panic せず、repaint 要求は立つ |
| 6-7 | `immediately_ready_future_lands_on_the_next_frame` | `async { 1 }` のような即完了 future。初回 `run` は `Pending`、repaint 待ちの後の `run` で `Ready(1)` |
| 6-8 | `spawn_with_dispatch_lands` | `spawn(async move { dispatch.send(Msg::Add(n)) })` で reducer に届き、repaint が要求される |
| 6-9 | `pending_is_counted_by_the_nearest_boundary` | `begin_suspense` / `end_suspense` を直接使い、Pending 2 つ + Ready 1 つで 2 が返る。入れ子の内側の Pending は外側に数えられない |

### 3.2 `crates/react-egui-elements/tests/suspense.rs`

`react-egui-elements/tests/common` のランナーを使い、`max_passes` は 3。

| # | テスト | 確認すること |
|---|---|---|
| 6-10 | `fallback_while_pending_then_children` | 子 2 つが `Pending` の間は fallback のラベルだけが見え、children のラベルは `query_by_label` で見つからない。両方完了して `run` すると children が見え fallback は消える |
| 6-11 | `partial_children_are_never_shown` | 片方だけ完了した状態で `run` しても fallback のまま |
| 6-12 | `switch_happens_in_one_frame` | 完了後の最初のフレームを `step` で 1 回だけ進め、children が見える(`request_discard` による同一フレームの切り替え)。`current_pass_index` が 1 以上 |
| 6-13 | `children_state_survives_suspension` | children の `use_state` カウンタをボタンで増やしてから deps を変えて再 suspended にし、解けた後もカウンタの値が残っている |
| 6-14 | `nested_boundary_catches_its_own` | 外側の children は Ready、内側だけ Pending。外側の children は見え、内側だけ fallback |
| 6-15 | `inside_a_view_children_lay_out_in_the_parent_tree` | `<View direction="row"><Suspense>..<View grow>..</Suspense></View>` で、解けた後の子 `<View>` が親の行の幅を埋める(`shares_ui` で surface を引き継いでいることの確認) |
| 6-16 | `offscreen_children_do_not_react_to_input` | suspended 中に children のボタンの位置をクリックしても children の state が変わらない |

wasm 経路のコンパイルは CI の `cargo check --workspace --target wasm32-unknown-unknown` と fetch の `trunk build` で固定する。実行は目視。

## 4. `examples/fetch`

`examples/counter` を複製して `fetch` に改名する(`Cargo.toml` / `index.html` / `Trunk.toml` / `src/main.rs`)。依存に `ehttp`(feature `native-async`)を足す。

```rust
#[component]
fn App(cx: &mut Cx) {
    let mut url = use_state(cx, || String::from("https://httpbin.org/get"));
    let mut attempt = use_state(cx, || 0u32);

    rsx! {
        <View direction="column" gap={8} p={12} grow={1.0}>
            <View direction="row" gap={8} align="center">
                <TextEdit grow={1.0} bind={url.bind()} on_submit={|_: String| *attempt += 1}/>
                <Button on_click={|| *attempt += 1}>"fetch"</Button>
            </View>
            <Suspense fallback={|cx: &mut Cx| { cx.ui().spinner(); }}>
                <Response url={url.as_str()} attempt={*attempt}/>
            </Suspense>
        </View>
    }
}

#[component]
fn Response(cx: &mut Cx, url: &str, attempt: u32) {
    let response = use_future(cx, (url, attempt), || {
        let request = ehttp::Request::get(url);
        async move { ehttp::fetch_async(request).await }
    });
    let Poll::Ready(response) = response else { return };
    match response {
        Ok(res) => rsx! {
            <Text strong>{format!("{} {}", res.status, res.status_text)}</Text>
            <ScrollArea grow={1.0}>
                <Text>{res.text().unwrap_or("(binary)").chars().take(2000).collect::<String>()}</Text>
            </ScrollArea>
        },
        Err(err) => rsx! { <Text>{format!("error: {err}")}</Text> },
    }
}
```

- deps は `(&str, u32)`。`Hash` なので借用のまま渡せる。`attempt` を混ぜて同じ URL の再取得を起こす。
- `let-else` で抜ければ `Suspense` がスピナーを出す。`Response` は自分では pending を描かない。
- `match` の 2 つの腕が違う `rsx!` 型を返すので、`View::show` を各腕で呼ぶ形(`rsx!{..}.show(cx)`)に直す必要があれば直す(`#[component]` の末尾式の書き換えが `match` を通すか実装時に確認)。
- `index.html` の `<title>` と `data-bin` を `fetch` にする。

## 5. CI / README

- `ci.yml`: `trunk build --release --config examples/fetch/Trunk.toml` を counter の後に足す。
- README の examples の一文に `fetch` (`use_future` + `Suspense` + `ehttp`) を足す。

## 6. ARCHITECTURE.md に反映する変更(着手時点で判明しているもの)

- **4 章の表** `use_future` の行を実際のシグネチャに合わせる: `use_future(cx, deps, || async { .. }) -> &Poll<T>`。「native はスレッド + `pollster`、wasm は `wasm_bindgen_futures::spawn_local`。完了時に `request_repaint`。deps 変更で作り直し、古い結果は捨てる。`Pending` は最も近い `<Suspense>` に数えられる」。`spawn(fut)` の行を足す(hook ではないが同じ表に)。
- **4 章に「`use_future` の詳細」節を追加** 1.2 〜 1.5 の内容(スロット 2 つ、`FrozenVec` に `Poll` を積む理由、世代番号、起動直後は受信しない、unmount 後の扱い、bound の platform 差を `SpawnFuture` に閉じ込める、`T: Send` を両 platform で要求する理由)。子コンポーネントの書き方は `let Poll::Ready(x) = .. else { return };`。
- **5 章に「5.8 Suspense」を追加** 2.1 / 2.2 の内容(カウンタのスタック、初期 suspended、オフスクリーン不可視 `Ui`、同じスコープ Id で描く理由、`request_discard` による同一フレーム切り替えと却下時の挙動、`use_effect` が走る差、React の throw との対応)。
- **5.6** 既に「`use_future` の完了は `request_repaint`」とある。変更なし(実装が一致していることを確認)。
- **6 章の要素一覧** `Suspense`(`shares_ui`、`fallback: impl View`)を足す。「パネルと `Row` が `shares_ui` である理由」の段落に `Suspense` を足す(surface を引き継ぐため)。
- **7 章** `react-egui` の依存に `pollster`(native)と `wasm-bindgen-futures`(wasm)。examples に `fetch`。
- **8 章** 「非同期の実行機構は core の `task::spawn` に閉じる。iOS / Android は native と同じスレッド経路」を 1 行足す。
- **11 章(決定ログ)** 行を 3 つ足す。
  - executor: スレッド + `pollster` を採用、tokio 必須を却下。理由: 依存が小さく、待つだけの future に十分。tokio を使うアプリは future の中で `Handle::current()` を使えばよい。
  - 結果の表現: `Poll<T>` を採用、独自の `Loading / Ready / Error` enum を却下。理由: std にあり、エラーは `T = Result` で表せる。
  - Suspense の実現: オフスクリーン描画 + カウンタ + `request_discard` を採用、panic / `catch_unwind` による巻き戻しを却下。理由: Rust に安価な巻き戻しが無く、let-else 1 行で足りる。

## 7. 手順

1. `future.rs`(`SpawnFuture` → `task::spawn` / `spawn` → `use_future`)、`store.rs` のカウンタ、re-export、`Cargo.toml`。テスト 6-1 〜 6-9。`cargo check --target wasm32-unknown-unknown -p react-egui` を通す。ARCHITECTURE.md 4 / 7 / 8 / 11 を更新。コミット。
2. `suspense.rs` と `prelude`。テスト 6-10 〜 6-16。ARCHITECTURE.md 5.8 / 6 を更新。コミット。
3. `examples/fetch`。`cargo run -p fetch` と `trunk serve` を目視(取得中はスピナーだけ、UI が動く、完了後に勝手に更新される、URL を変えて Enter で再取得、fetch ボタンで同じ URL を再取得)。CI と README。コミット。
4. CI が全ステップ緑であることを確認する。
5. PR 本文に、成果、ARCHITECTURE.md の変更点、外したもの(あれば)を書く。

## 8. 判断が必要になりそうな点

- **`impl FnOnce() -> impl SpawnFuture<T>` の推論**(1.1)。`async move { .. }` ブロックの `Send` 判定で `State` の guard や `&str` を跨いで await していると native でコンパイルできない。エラー文面が分かりにくければ、rustdoc に「future に入れるのは clone した値だけ」と例を書く。マクロ側での支援はしない。
- **kittest とスレッドの待ち合わせ**(3.1)。`wait_for_repaint` の sleep ループが CI で不安定なら、future の中で書き込みの後に `tx.send(())` を置いて「書き込み済み」をテスト側に知らせる形にする(書き込みと `request_repaint` の順を固定すれば、`send` を受けた後の `has_requested_repaint` は決定的)。
- **オフスクリーン `Ui` と egui_taffy**(2.2)。`Ui::new` で作った root `Ui` の中で `<View>`(egui_taffy)が毎パス `request_discard` するようなら、矩形を固定するだけでなく `sizing_pass()` を外す、あるいは children を `ui.new_child(..)` で親の `Ui` にぶら下げる(場所は取らない)に切り替える。`max_passes` を食い潰して切り替えが次フレームに落ちるのは許容する。
- **`#[component]` の末尾式が `match` の場合**(4 章)。腕ごとに型が違う `rsx!` を返せないなら、各腕で `.show(cx)` を呼ぶ形に書く。マクロは直さない。
- **`ehttp` の feature**。native の `fetch_async` は `native-async`(`async-channel` を引く)。wasm 側で余計な依存を引かないよう、`[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` に分けて書く。trunk ビルドが通るまで確認する。
- **httpbin の可用性**。example の既定 URL は目視用なので落ちていても構わないが、README には「任意の URL を入れて Enter」と書く。
- **`prelude` の `Poll` re-export**。ユーザーの `Poll` と衝突する報告があれば外し、`std::task::Poll` の `use` を README の例に書く。

## 9. 実装で判明した差分

(実装中に書く)

## 10. PR 本文の材料

(実装中に書く)
