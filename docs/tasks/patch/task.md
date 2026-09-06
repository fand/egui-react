# タスク: patch(PR8 = フェーズ 6.6 の後半。[board](../board/task.md) と 1 つの PR)

## 目的

TouchDesigner 風のノードエディタで、**複雑な UI がそのまま egui-react で組める**ことを示す。[board](../board/task.md) が 1 つずつ証明した性質(ローカル state、key、自作コンポーネント、custom hook、context)が、3 ペイン + ノードグラフ + GPU プレビューという規模の画面でも崩れないことを見せるのが役目である。

本格的な VJ アプリを作ることは目的ではない。ノードは「シェーダを書けるノード + 定番のエフェクト 5 つ」に絞る。

技術的な芯は **グラフ → WGSL の生成 → 1 本の fragment shader** である。オフスクリーンの合成パスを持たないので実装が軽く、しかも egui-react の主張と噛み合う。

- グラフの形が変わった時だけ `use_memo` が WGSL を作り直し、それが変わった時だけ pipeline を作り直す。
- スライダを動かしただけならシェーダは再生成されず、値が uniform に流れるだけ。
- 「state が変わると絵が変わる」経路が 2 段階あり、どちらも派生(memo)としてコードに現れる。

board が作る `use_dnd` をノードの移動とポートの結線に再利用する。これが board を先にする理由でもある。

core(`egui-react`、`egui-react-macros`)には手を入れない。

## スコープ

### 含む

- `examples/patch`(lib + bin、生 egui 版なし)。
- 画面: 左にノードパレット、中央にパッチキャンバス(パン、ノードのドラッグ、結線)、右に出力プレビューとインスペクタ(縦積み)。
- ノード種(いずれも入力 0〜2、出力 1):
  - `Shader`: WGSL の式を直接書ける。ソースにもなる。パラメータ 3 つを式から参照できる。
  - `Level`: brightness / contrast / gamma。
  - `Mix`: 2 入力。ブレンド方法(mix / add / multiply / screen / difference)と amount。
  - `HSV`: 色相 / 彩度 / 明度。
  - `Transform`: UV 側にかかる平行移動 / 回転 / 拡大。
  - `Grayscale`: luma / average / max。
  - `Output`: 終端。1 つだけ。
- ノードのローカル state(折りたたみ、名前のインライン編集、ドラッグ中のオフセット、ポートの hover)はノード自身の `use_state`。
- ノード種による多態レンダリング。ノード本体もインスペクタも `match kind` で別コンポーネントに分かれる。
- 派生の連鎖: グラフ → トポロジ順 → WGSL 生成 → naga での検証 → pipeline。`use_memo` を段ごとに置く。検証エラーはインスペクタに文言として出す。
- 結線: ポートから引っぱって別のポートに落とす。board の `use_dnd` を payload 型だけ替えて使う。ワイヤは `<Canvas>` の painter で描く。
- プレビュー: `<Canvas>` + `egui_wgpu::Callback`。`shader` example と同じく `callback_resources` に型で置く。
- プリセットのパッチを 1 つ `use_future` で読み、`<Suspense>` でプレビューだけがスピナーになる(画面の一部だけが待つことの実演。この example で非同期を使うのはここだけ)。
- グラフは `use_reducer` + `use_persisted`。undo / redo は board の `use_undoable` を再利用する。
- gallery への登録、kittest、README の表、ARCHITECTURE の更新(必要なら)。

### 含まない

- 生 egui 版。この規模で同じものを 2 度書く価値は無く、差を見せる役目は board が持つ。
- オフスクリーンのレンダーターゲット、複数パス、フィードバック、テクスチャ / 動画 / カメラ入力。ソースは `Shader` ノードだけ。
- ノードのコピー & ペースト、グループ化、コメント、自動整列、ミニマップ、複数選択のボックス選択。
- 保存フォーマットの互換性、ファイルの読み書き、OSC / MIDI。
- 実行時のホットリロード、シェーダのエラー箇所のハイライト(文言を出すところまで)。
- core の変更。必要が出たら plan.md に書き、別 PR に切る。

## 成果物

- `examples/patch/`(`src/lib.rs` / `graph.rs` / `codegen.rs` / `gpu.rs` / `main.rs` / `tests/patch.rs` ほか)。
- `examples/gallery` への登録(`EXAMPLES`、`Running` の `match`、`setup` への追加)。
- README の表に 1 行。plan.md の「実装で判明した差分」。

## 終了条件

- kittest: ノードを追加 → 結線 → パラメータ変更で、生成された WGSL が期待どおりに変わる(文字列として検証する)。循環を作ると検証エラーになり、直前の pipeline のまま落ちない。ノードを並べ替え / 削除しても、残ったノードの折りたたみ状態と名前の下書きが付いて回る(board B-2 と同じ性質を、より深いツリーで)。
- `codegen` のユニットテスト: 各ノード種 1 つずつ、Transform の入れ子、DAG の共有、循環の検出。
- `cargo run -p patch` でプレビューが動き、スライダで絵が変わり、`Shader` ノードの式を書き換えると再コンパイルされる(目視)。`trunk serve` でブラウザでも同じ(目視。WebGPU 非対応ブラウザは WebGL fallback で確認する)。
- gallery に `patch` が載り、`shader` と同居して `callback_resources` が衝突しない。
- CI(fmt / clippy / test / wasm check / trunk ループ)が緑。

## 決めごと(着手時点での前提)

- **ノードは `fn n<id>(uv: vec2<f32>) -> vec4<f32>` として生成し、`Output` から辿って呼ぶ。** `Transform` は uv を変換してから入力を呼ぶので、共通部分式の巻き上げはしない(uv が違えば結果も違う)。同じ出力が 2 箇所へ繋がっていれば 2 回呼ばれるが、正しさは変わらない。
- **パラメータは uniform、トポロジは再コンパイル。** uniform は `array<vec4<f32>, 32>` の固定長で、各ノードのスロット番号を codegen 時に焼き込む。ブレンド方法や Grayscale の方法は式が変わるので再コンパイル側。
- pipeline は `Options::setup` では作らない(グラフが決まっていない)。`setup` は空の `PatchResources` を置くだけにし、`CallbackTrait::prepare` で WGSL のハッシュが変わった時に作り直す。作れなかった場合は直前の pipeline で描き続ける。
- WGSL の検証は pipeline を作る前に naga で行い、通った時だけ `create_shader_module` する。ユーザーが書いた式でパイプライン生成が落ちるのを避け、エラーは UI の文言にする。
- ノードの配置は絶対座標で、`ItemStyle` に `position: absolute` は無い。パッチキャンバスは escape hatch(ノードごとに `max_rect` を与えた子 `Ui` を作り、その中で `Cx` を作り直す)で置き、**ノードの内部は普通に `<View>` で組む**(ARCHITECTURE 3.1 と `escape-hatch` example の形)。`ItemStyle` への絶対配置の追加は、書けないと分かった時に初めて検討する。
- ズームは入れるなら `Context::set_transform_layer` に寄せる。手が要るようなら省き、パンだけにする。
- gallery に載せるので `Panel` / `CentralPanel` を使わず、渡された領域を埋める。`std::time::Instant` は使わない。`use_persisted` のキーは `"patch/..."`。

## 手順

board と 1 つの PR にまとめる。board を終了条件まで仕上げてから patch に入る。`use_dnd` と自作コンポーネントの形は board で確定するので、[plan.md](plan.md) の 5 章はその時点の実物に合わせて更新する。
