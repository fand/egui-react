# タスク: async(PR3 = フェーズ 6)

## 目的

core(PR2)の上に非同期の土台を足す。hook `use_future` で「フレームをまたぐ処理」をコンポーネント本体から `async` ブロック 1 つで書けるようにし、結果が届いたら勝手に再描画されるようにする。加えて React の `<Suspense>` に相当する `<Suspense fallback=..>` 要素を足し、境界の中の pending をまとめて 1 つの fallback に置き換えられるようにする。native と wasm で同じコードが動くこと(実行機構の違いは core の中に閉じ込める)を確認するため、両方で動く `fetch` example を足す。

PR2 で `Dispatch` を `Send + 'static` にしてある。本 PR はその上に「完了時に結果をスロットへ書き戻して `request_repaint` する」経路と、`Store` の suspense カウンタを足す。`Cx` / マクロには手を入れない。

## スコープ

### 含む

- `use_future(cx, deps, || async { .. }) -> &Poll<T>`(core、`future.rs`)。
  - deps のハッシュが変わるたびに future を作り直して起動する。比較は `use_effect` / `use_memo` と同じ Hash。
  - 起動した future は native ではスレッド 1 本で `pollster::block_on`、wasm では `wasm_bindgen_futures::spawn_local`。
  - 完了時に結果をスロットの共有セルに書き、`request_repaint` する。hook は次の訪問でセルを読み `Poll::Ready(T)` に切り替える。
  - deps が変わった後に届いた古い結果は捨てる(世代番号)。走っている future は止めない(止められない)。
  - 返り値は `use_memo` と同じ `&'s Poll<T>`。`State` の guard と同時に生きる。
  - 同一フレームの 2 パス目で二重起動しない。unmount 後に届いた結果で panic しない。
  - `Pending` を返すとき、`Store` の suspense カウンタ(最も近い `<Suspense>` のもの)を +1 する。
- `react_egui::spawn(future)`: core の `task::spawn` を公開する。`Dispatch` と組み合わせて「命令的に起動して結果を送る」(mutation、optimistic update)を書けるようにする。
- native / wasm の実行機構の差を `SpawnFuture<T>` trait(bound の cfg 切り替え)と `task::spawn` に閉じ込める。
- `Store` の suspense カウンタのスタック(`provide_context` のスタックと同じ形)。
- `<Suspense fallback={..}>children</Suspense>`(elements、`suspense.rs`)。
  - 境界の中で 1 つでも `use_future` が `Pending` なら children の代わりに `fallback` を描く。全部 `Ready` になったら children を描く。
  - 切り替えは `request_discard` で同一フレーム内に行い、children の描きかけが見えない。
  - suspended の間も children はオフスクリーンの不可視 `Ui` に描き続ける(hooks が走り、future が起動・完了する)。
  - 入れ子は最も近い境界が拾う。
  - `fallback` は `impl View`(`rsx!{..}` / 閉包 / `&str`)。
- 子コンポーネントの書き方は `let Poll::Ready(x) = use_future(..) else { return };`。React の throw の代わり。
- `examples/fetch`: `ehttp::fetch_async` で URL を GET する。`<Suspense>` の中の `Response` コンポーネントが let-else で待ち、fallback はスピナー。完了後にステータスと本文の先頭を表示する。native と trunk の両方で動く。「再取得」ボタンは deps に混ぜたカウンタを増やす。
- kittest テスト(plan.md の表)。
- CI: `trunk build` に fetch を足す(wasm の `spawn_local` 経路が実際にリンクできることの確認)。
- README の examples 一覧に fetch を足す。ARCHITECTURE.md の 4 章(`use_future` の行と「詳細」節)、5 章(Suspense の節)、6 章(要素一覧)、7 章、8 章、11 章(決定ログ)を更新。

### 含まない

- tokio 連携。native の既定はスレッド + `pollster` で、tokio を使いたいアプリは future の中で `Handle::current().spawn(..).await` する。executor の差し替え口(`Store::set_spawner` のようなもの)は要望が出てから足す。
- future のキャンセル(`AbortHandle` 相当)。deps 変更と unmount では結果を捨てるだけで、走っている処理は完走させる。
- `use_query`(key 付きキャッシュ、コンポーネント間共有、stale-while-revalidate)、`use_action`(命令的起動 + pending)、`use_debounced`、`use_stream`。次の PR(async-2)。`use_future` の中身を `AsyncSlot` として切り出すのもその時。
- `Poll` 以外の状態表現(`Loading` / `Error` の enum など)。エラーは `T = Result<..>` で表す。
- Error boundary。エラーは値なので子で `match` する。
- `Suspense` の suspended 中に子の `use_effect` を止めること(React は commit しないので走らない)。react-egui では走る。ドキュメントに書く。
- `SuspenseList`、`useTransition` 相当。
- `Spinner` 要素。fetch example は `cx.ui().spinner()` を閉包で呼ぶ。
- PR2 からの持ち越し(`use_persisted_reducer`、`use_persisted` の wasm 自動テスト、`App::save` の dirty フラグ)。別 PR。
- Android / iOS(PR4)。英語ドキュメント、API の見直し、crates.io 公開(フェーズ 8)。

## 成果物

- `crates/react-egui/src/future.rs`(`use_future`、`SpawnFuture`、`spawn`)、`store.rs` の suspense カウンタ、`lib.rs` / `prelude` の再エクスポート(`use_future`、`spawn`、`std::task::Poll`)。
- `crates/react-egui/tests/future.rs`(kittest)。
- `crates/react-egui-elements/src/suspense.rs`(`Suspense`)と `prelude` への追加、`crates/react-egui-elements/tests/suspense.rs`。
- `examples/fetch`(`Cargo.toml` / `src/main.rs` / `index.html` / `Trunk.toml`)。
- `.github/workflows/ci.yml` に fetch の trunk ビルドを追加。
- README の examples の一文を更新。
- `docs/ARCHITECTURE.md` の更新(着手時点で判明している変更点は [plan.md](plan.md) 6 章、実装中に判明したものはその都度)。

## 終了条件

- [plan.md](plan.md) のテスト表がすべて緑。PR2 までのテストがすべてそのまま通る。
- `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo check --workspace --target wasm32-unknown-unknown`、`trunk build`(counter と fetch)が CI で通る。
- `cargo run -p fetch` で URL を取得でき、取得中はスピナーだけが見え(描きかけの本文が一瞬も見えない)、UI が固まらず、完了後にマウスを動かさなくても画面が更新される(目視)。
- `trunk serve --config examples/fetch/Trunk.toml` でブラウザでも同じ挙動(目視)。
- ARCHITECTURE.md が実装と一致している。変更点は PR 本文に列挙する。

## 決めごと(着手時点での前提)

- native の executor はスレッド 1 本 + `pollster::block_on`。future 1 つにつきスレッド 1 本で、プールは作らない。理由: 依存が最小で、`ehttp` / ファイル IO のような「待つだけ」の future に十分。CPU を食う処理は future の中で自分でスレッドを分ける。
- future の bound は native で `Future<Output = T> + Send + 'static`、wasm で `Future<Output = T> + 'static`。`T` は両方で `Send + 'static`(wasm で `JsValue` を返したい場合は future の中で `Send` な型に変換する)。この差は `SpawnFuture<T>` trait に閉じ込め、ユーザーの書く型には出さない。
- 結果の受け渡しは `Arc<Mutex<Option<(u64, T)>>>`(世代付き)。`Dispatch` のキューと同じ形で、poison は無視する。
- 返り値は `&'s Poll<T>`。値は `use_memo` と同じ `FrozenVec` に積む(`Pending` を 1 つ、届いたら `Ready` を 1 つ)。同じパスで先に配った参照が古い値を指していてもよいよう、パス末の sweep で最新の 1 つを残す(既存の `prune_memo`)。
- deps を変えて future を作り直すのは訪問時。走っている古い future は完走するが結果は世代不一致で捨てる。unmount ではスロットが消えるので、届いた結果は誰も読まず `request_repaint` が 1 回余分に飛ぶだけ。
- `Suspense` は elements に置く(`Collapsing` と同じ「子を包む要素」)。core に足すのは suspense カウンタの 3 メソッドだけ。
- `Suspense` の初期状態は suspended(初回はオフスクリーンに描いてから、pending が無ければ discard して可視に切り替える)。理由: `max_passes` を使い切って discard が却下された時に、描きかけの children ではなく fallback が見える側に倒す。
- オフスクリーン描画は `egui::Ui::new(ctx, id, UiBuilder::new().max_rect(画面外の大きな矩形).invisible().sizing_pass())`。`invisible()` は描画と操作の両方を無効にする(egui 0.36 で確認)。親の `Ui` から場所を取らない。
- suspended / 可視のどちらでも children は同じスコープ Id(`Suspense` 自身の `scope_id()`)で描く。hook のスロットが両経路で共有されることが、切り替えで state と future が保たれる根拠。
- example の HTTP クライアントは `ehttp`(native は ureq、wasm は fetch API。`fetch_async` が future を返す。native は feature `native-async` が要る)。`reqwest` は tokio が要るので使わない。
- 依存の追加: `pollster`(core、native のみ)、`ehttp`(examples/fetch)。バージョンは着手時の最新を `[workspace.dependencies]` に pin する。
