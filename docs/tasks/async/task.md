# タスク: async(PR3 = フェーズ 6)

## 目的

core(PR2)の上に非同期 hook `use_future` を 1 つ足す。ネットワークやファイル読み込みのような「フレームをまたぐ処理」を、コンポーネント本体から `async` ブロック 1 つで書けるようにし、結果が届いたら勝手に再描画されるようにする。native と wasm で同じコードが動くこと(実行機構の違いは core の中に閉じ込める)を確認するため、両方で動く `fetch` example を足す。

PR2 で `Dispatch` を `Send + 'static` にしてある。本 PR はその上に「完了時に結果をスロットへ書き戻して `request_repaint` する」経路を足すだけで、ストア / `Cx` / マクロには手を入れない。

## スコープ

### 含む

- `use_future(cx, deps, || async { .. }) -> &Poll<T>`(core、`future.rs`)。
  - deps のハッシュが変わるたびに future を作り直して起動する。比較は `use_effect` / `use_memo` と同じ Hash。
  - 起動した future は native ではスレッド 1 本で `pollster::block_on`、wasm では `wasm_bindgen_futures::spawn_local`。
  - 完了時に結果をスロットの共有セルに書き、`request_repaint` する。hook は次の訪問でセルを読み `Poll::Ready(T)` に切り替える。
  - deps が変わった後に届いた古い結果は捨てる(世代番号)。走っている future は止めない(止められない)。
  - 返り値は `use_memo` と同じ `&'s Poll<T>`。`State` の guard と同時に生きる。
  - 同一フレームの 2 パス目で二重起動しない。unmount 後に届いた結果で panic しない。
- native / wasm の実行機構の差を `SpawnFuture<T>` trait(bound の cfg 切り替え)と `task::spawn` に閉じ込める。
- `examples/fetch`: `ehttp::fetch_async` で URL を GET し、`Pending` の間はスピナー、完了後にステータスと本文の先頭を表示する。native と trunk の両方で動く。「再取得」ボタンは deps に混ぜたカウンタを増やす。
- kittest テスト(plan.md の表)。
- CI: `trunk build` に fetch を足す(wasm の `spawn_local` 経路が実際にリンクできることの確認)。
- README の examples 一覧に fetch を足す。ARCHITECTURE.md の 4 章(`use_future` の行と「詳細」節)、7 章、8 章、11 章(決定ログ)を更新。

### 含まない

- tokio 連携。native の既定はスレッド + `pollster` で、tokio を使いたいアプリは future の中で `Handle::current().spawn(..).await` する。executor の差し替え口(`Store::set_spawner` のようなもの)は要望が出てから足す。
- future のキャンセル(`AbortHandle` 相当)。deps 変更と unmount では結果を捨てるだけで、走っている処理は完走させる。
- `use_future` の再試行 API、`Poll` 以外の状態表現(`Loading` / `Error` の enum など)。エラーは `T = Result<..>` で表す。
- ストリーム(`use_stream`)、複数回結果が届く形。
- `Dispatch` から future を起動する補助(`dispatch.spawn(..)`)。
- PR2 からの持ち越し(`use_persisted_reducer`、`use_persisted` の wasm 自動テスト、`App::save` の dirty フラグ)。別 PR。
- Android / iOS(PR4)。英語ドキュメント、API の見直し、crates.io 公開(フェーズ 8)。

## 成果物

- `crates/react-egui/src/future.rs`(`use_future`、`SpawnFuture`、`task::spawn`)と `lib.rs` / `prelude` の再エクスポート(`use_future`、`std::task::Poll`)。
- `crates/react-egui/tests/future.rs`(kittest)。
- `examples/fetch`(`Cargo.toml` / `src/main.rs` / `index.html` / `Trunk.toml`)。
- `.github/workflows/ci.yml` に fetch の trunk ビルドを追加。
- README の examples の一文を更新。
- `docs/ARCHITECTURE.md` の更新(着手時点で判明している変更点は [plan.md](plan.md) 5 章、実装中に判明したものはその都度)。

## 終了条件

- [plan.md](plan.md) のテスト表がすべて緑。PR2 までのテストがすべてそのまま通る。
- `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo check --workspace --target wasm32-unknown-unknown`、`trunk build`(counter と fetch)が CI で通る。
- `cargo run -p fetch` で URL を取得でき、取得中に UI が固まらず、完了後にマウスを動かさなくても画面が更新される(目視)。
- `trunk serve --config examples/fetch/Trunk.toml` でブラウザでも同じ挙動(目視)。
- ARCHITECTURE.md が実装と一致している。変更点は PR 本文に列挙する。

## 決めごと(着手時点での前提)

- native の executor はスレッド 1 本 + `pollster::block_on`。future 1 つにつきスレッド 1 本で、プールは作らない。理由: 依存が最小で、`ehttp` / ファイル IO のような「待つだけ」の future に十分。CPU を食う処理は future の中で自分でスレッドを分ける。
- future の bound は native で `Future<Output = T> + Send + 'static`、wasm で `Future<Output = T> + 'static`。`T` は両方で `Send + 'static`(wasm で `JsValue` を返したい場合は future の中で `Send` な型に変換する)。この差は `SpawnFuture<T>` trait に閉じ込め、ユーザーの書く型には出さない。
- 結果の受け渡しは `Arc<Mutex<Option<(u64, T)>>>`(世代付き)。`Dispatch` のキューと同じ形で、poison は無視する。
- 返り値は `&'s Poll<T>`。値は `use_memo` と同じ `FrozenVec` に積む(`Pending` を 1 つ、届いたら `Ready` を 1 つ)。同じパスで先に配った参照が古い値を指していてもよいよう、パス末の sweep で最新の 1 つを残す(既存の `prune_memo`)。
- deps を変えて future を作り直すのは訪問時。走っている古い future は完走するが結果は世代不一致で捨てる。unmount ではスロットが消えるので、届いた結果は誰も読まず `request_repaint` が 1 回余分に飛ぶだけ。
- example の HTTP クライアントは `ehttp`(native は ureq、wasm は fetch API。`fetch_async` が future を返す)。`reqwest` は tokio が要るので使わない。
- 依存の追加: `pollster`(core、native のみ)、`ehttp`(examples/fetch)。バージョンは着手時の最新を `[workspace.dependencies]` に pin する。
