# プラン: patch

タスク定義は [task.md](task.md)。設計の根拠は [docs/ARCHITECTURE.md](../../ARCHITECTURE.md)。[board](../board/plan.md) と 1 つの PR で、board を仕上げてからここに入る。実装中にここから外れる判断をした場合は本書を更新し、設計上の意味があれば ARCHITECTURE.md も更新する。

## 0. 全体

```
examples/patch/src/
  lib.rs      App と 3 ペイン。パッチキャンバスとノードもここ(gallery が見せるのはこれ)
  graph.rs    Graph / Node / Kind / Msg / reduce。純関数、ユニットテスト付き
  codegen.rs  Graph -> WGSL の文字列。naga での検証。純関数、ユニットテスト付き
  gpu.rs      setup(cc)、PatchResources、PatchCallback: CallbackTrait
  preset.rs   起動時に読む既定のパッチ(JSON を include_str!)
  main.rs     run(Options { setup: Some(Box::new(gpu::setup)), .. }, ..)
```

`codegen.rs` が GPU を一切知らないのが要点。文字列を作る純関数なので、テストは文字列の比較で書ける。

## 1. データモデル(`graph.rs`)

```rust
pub type NodeId = u64;

pub struct Graph {
    pub nodes: Vec<Node>,          // 描画順 = z 順。掴んだノードを末尾へ移す
    next_id: u64,
    /// 生成される WGSL が変わりうる変更でだけ +1。`use_memo` の deps。
    pub topology_rev: u64,
    /// パラメータだけの変更で +1。uniform にしか影響しない。
    pub param_rev: u64,
}

pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub kind: Kind,
    pub pos: egui::Pos2,           // グラフ座標
    pub inputs: [Option<NodeId>; 2],
    /// uniform の 1 スロット。意味は Kind ごとに違う。
    pub params: [f32; 4],
}

pub enum Kind {
    Shader { src: String },                 // src は topology 側
    Level, Hsv, Grayscale { method: GrayMethod },
    Transform,
    Mix { mode: MixMode },
    Output,
}

pub enum Msg {
    AddNode { kind: Kind, pos: egui::Pos2 },
    RemoveNode(NodeId),
    MoveNode { node: NodeId, pos: egui::Pos2 },
    Connect { from: NodeId, to: NodeId, port: usize },
    Disconnect { to: NodeId, port: usize },
    SetParam { node: NodeId, index: usize, value: f32 },
    SetShaderSrc { node: NodeId, src: String },
    SetMode { node: NodeId, mode: MixMode },
    SetGray { node: NodeId, method: GrayMethod },
    Rename { node: NodeId, name: String },
    Raise(NodeId),
}
```

**`topology_rev` と `param_rev` を分けるのがこの example の芯。** `reduce` は変更の種類でどちらを上げるか決める(`SetParam` と `MoveNode` と `Raise` は param 側、それ以外は topology 側)。スライダを動かしても WGSL が再生成されないことは、テスト P-5 で固定する。

`Output` は 1 つだけ。`AddNode` では作らせず、初期グラフが持つ。

## 2. WGSL の生成(`codegen.rs`)

```rust
pub struct Generated { pub wgsl: String, pub slots: Vec<NodeId> }
pub fn generate(graph: &Graph) -> Result<Generated, GenError>;
pub enum GenError { Cycle(NodeId), TooManyNodes, Wgsl(String) }
```

- 各ノードは `fn n<id>(uv: vec2<f32>) -> vec4<f32>` になり、`Output` から辿れるものだけを、依存が先に来る順(トポロジ順。WGSL は前方参照できない)で出す。
- 入力が繋がっていないポートは `vec4<f32>(0.0, 0.0, 0.0, 1.0)`。`Output` に何も繋がっていなければ黒を返す関数だけを出す。
- 同じ出力が 2 箇所に繋がっていれば 2 回呼ばれる。`Transform` が uv を変えるので巻き上げはしない(task.md の決めごと)。
- 循環は生成前に DFS で検出し `GenError::Cycle`。
- uniform:

```wgsl
struct Uniforms {
    time: f32,
    _pad: f32,
    resolution: vec2<f32>,
    p: array<vec4<f32>, 32>,   // ノードのスロット。添字は codegen が焼き込む
};
@group(0) @binding(0) var<uniform> U: Uniforms;
```

  `slots[i] == node_id` を返し、UI 側はこの順で `params` を詰める。ノードが 32 を超えたら `TooManyNodes`(パレット側で足せなくする)。
- ノード種ごとの本体(`p` はそのノードのスロット):

| Kind | params | 本体 |
|---|---|---|
| `Shader { src }` | p0 p1 p2 | `let t = U.time; let p0 = ..; return <src>;` |
| `Level` | brightness contrast gamma | `pow(clamp((c-0.5)*contrast+0.5+brightness, 0, 1), vec3(1/max(gamma, 1e-3)))` |
| `Hsv` | hue sat val | prelude の `rgb2hsv` / `hsv2rgb` を通す |
| `Grayscale { method }` | – | luma `dot(c, vec3(0.2126, 0.7152, 0.0722))` / average / max。方法は焼き込み |
| `Transform` | tx ty rot scale | uv を中心 0.5 で回転・拡大・平行移動してから入力を呼ぶ |
| `Mix { mode }` | amount | mix / add / multiply / screen / difference。mode は焼き込み |
| `Output` | – | 入力をそのまま返す |

- prelude(頂点シェーダの fullscreen triangle、`rgb2hsv` / `hsv2rgb`、`fs_main`)は `include_str!("prelude.wgsl")` で、生成部分と連結する。`fs_main` は `n<output>(uv)` を呼ぶだけ。
- 検証は **CPU 側で naga**(`naga::front::wgsl::parse_str` → `naga::valid::Validator`)。GPU も device も要らないので `use_memo` の中で回り、エラーはそのまま UI の文言になる。`wgpu::naga` が使えるならそれを、駄目なら wgpu 30 が使う版を workspace に pin する。ユーザーが書いた式の誤りはここで止まり、pipeline 生成まで行かない。

## 3. GPU 側(`gpu.rs`)

`shader` example と同じ形。違いは pipeline を起動時に作らないことだけ。

```rust
pub struct PatchResources {
    buffer: wgpu::Buffer, bind_group: wgpu::BindGroup, layout: wgpu::PipelineLayout,
    target_format: wgpu::TextureFormat,
    pipeline: Option<wgpu::RenderPipeline>,
    source_hash: u64,
}
pub fn setup(cc: &eframe::CreationContext<'_>);   // buffer / bind group / layout まで
pub struct PatchCallback { pub wgsl: Arc<str>, pub source_hash: u64, pub uniforms: Uniforms }
```

- `prepare` で `source_hash` が違えば `create_shader_module` + `create_render_pipeline` して差し替える。失敗しても直前の pipeline を残す(絵が黒く落ちない)。naga で通したものしか来ないので、ここで落ちるのは想定外の場合だけ。
- uniform は毎フレーム `queue.write_buffer`。`paint` は 3 頂点。
- `callback_resources` は型で引くので gallery で `shader` と同居できる(`shader` example のコメントと同じ理屈)。

## 4. 画面(`lib.rs`)

```
App
└ PatchProvider              Dispatch とテーマを provide_context(board と同じ形)
  └ View row grow
    ├ Palette (w=140)        ノード種のボタン。クリックで中央に追加、ドラッグで位置指定
    ├ View column grow       ★ パッチキャンバス(下の 4.1)
    └ View column (w=280)
      ├ Preview              <Canvas> + wgpu callback。再生 / 一時停止
      └ Inspector            タブ: params / wgsl
```

- `Inspector` の `params` は選択ノードの `match kind` で別コンポーネント(`LevelParams` `MixParams` ..)。`wgsl` タブは生成されたコードとエラーをそのまま出す。**これは見せるための機能であり、テストが生成結果を UI から読む口でもある**(6 章)。
- 派生の連鎖:

```rust
let gen = use_memo(cx, graph.topology_rev, || codegen::generate(&graph));   // Result
let wgsl: Option<Arc<str>> = ..;      // Ok の時だけ差し替え、Err なら直前を保つ
let params = use_memo(cx, (graph.param_rev, graph.topology_rev), || pack_uniform_slots(&graph, &gen));
```

  スライダを動かした時に走るのは 2 本目だけである。この 2 段構造を `lib.rs` の冒頭コメントで説明する。

### 4.1 パッチキャンバス

ノードの配置は絶対座標で、`ItemStyle` に絶対配置は無い。`escape-hatch` example の `Nested` と同じ形で降りる。

```rust
let (store, scope) = (cx.store, cx.scope_id());
// <View grow={1.0} h={0.0}> の中の {view(move |cx| cx.leaf_fill(&style, move |ui| { .. }))}
//   1. ui.allocate_exact_size(available, Sense::click_and_drag()) で領域と背景の応答
//   2. 背景のドラッグでパン(use_state<Vec2>)
//   3. ワイヤを ui.painter() でベジエで引く(ポート位置はノード矩形から計算するので測定が要らない)
//   4. ノードごとに:
//        ui.scope_builder(egui::UiBuilder::new().max_rect(node_rect), |ui| {
//            let mut cx = Cx::new(store, ui, scope);
//            cx.scope(node.id, |cx| NodeView(cx, props));   // ← これが key
//        });
```

- `cx.scope(node.id, ..)` が rsx! の `key={..}` に当たる。ここだけ手で書くので、**key の正体が「スコープ Id に混ぜるもの」であることがコードに出る**。コメントで board の `key=` と結び付ける。
- ノードの中は普通に `<View>`。Ui モード直下の `container` は新しい taffy ツリーを開くので、ヘッダ行(名前 + 折りたたみ)とパラメータの縦並びはフレックスで組める。
- ノードのローカル state: `collapsed` / `editing_name` / `draft_name` / `hovered_port`。**ノードを消しても残りのノードの状態が付いて回る**ことをテスト P-3 で固定する。
- z 順は `graph.nodes` の順。掴んだら `Msg::Raise` で末尾へ。
- ズームは入れない(task.md の決めごと)。パンのみ。

### 4.2 プリセットと Suspense

```rust
let loaded = use_future(cx, (), || async { preset::parse(preset::STARTER) });
let Poll::Ready(graph0) = loaded else { return };   // 最も近い <Suspense> が fallback を描く
```

`STARTER` は `include_str!` の JSON なので待ち時間はほぼ無いが、future は必ず 1 フレーム以上 `Pending` を通る。`<Suspense fallback={..}>` はプレビューとキャンバスだけを包み、パレットとツールバーは待たせない。**画面の一部だけが待つ**ことの実演がここでの役目で、実アプリなら fetch になる旨をコメントに書く。

## 5. board からの再利用

board の `hooks.rs`(`use_dnd` / `use_undoable`)をこの example から使う。方法は 2 つあり、board が終わった時点で決める。

- (a) `examples/board` を `patch` の依存にし、`board::hooks` を `pub` にして使う。追加のクレートが要らない。
- (b) `examples/hooks`(仮)を新しいクレートに切り出し、board と patch が両方依存する。

**(a) を既定とする。** gallery の example どうしが依存し合うのは行儀が悪いが、切り出しは「同じ hook が 2 つの画面で使い回せた」という主張を薄める。どちらにせよ、hook の中身は両方の example で 1 つである。(b) に倒すのは、board 側の hook が patch のために形を歪められる場合だけ。

`use_dnd<T>` は型引数を替えて 2 用途に使う。ノードの移動(`T = NodeId`)とポートの結線(`T = PortRef { node, port }`)。ポートの当たり判定はノード矩形から計算した円なので、slot の登録は board と同じ形で書ける。

## 6. テスト(`tests/patch.rs`)

kittest は headless(GPU 無し)で回す。`<Canvas>` に積む wgpu callback は描画されないだけで、UI の検証には影響しない(`shader` example と同じ)。

- **P-1** パレットからノードを追加するとキャンバスのノードが増え、インスペクタに出る。
- **P-2** 追加したノードを Output に繋ぐと、`wgsl` タブの本文にそのノードの関数と呼び出しが現れる。
- **P-3** ノードを 1 つ削除しても、残ったノードの折りたたみ状態と名前の下書きが付いて回る(board B-2 の性質を、より深いツリーで)。
- **P-4** 循環を作るとインスペクタにエラーが出て、`wgsl` タブは直前の内容のまま残る。
- **P-5(目玉)** パラメータのスライダを動かしても `wgsl` タブの本文が 1 文字も変わらない。ノードを繋ぎ替えると変わる。= 再コンパイルとパラメータ更新が分かれていることの証明。
- **P-6** プリセットの読み込みで `<Suspense>` の fallback が 1 度は出て、その後キャンバスにノードが並ぶ。
- `codegen` のユニットテスト: 各 Kind 1 つずつ、`Transform` の入れ子、同じ出力を 2 入力に繋いだ DAG、循環の検出、32 ノード超え。生成物は naga を通す(これで「生成した WGSL が常に妥当」がテストで担保される)。
- `graph` のユニットテスト: `reduce` が `topology_rev` / `param_rev` のどちらを上げるかを、全 `Msg` について 1 行ずつ。

## 7. gallery / README

- `examples/gallery/Cargo.toml` に依存を足し、`EXAMPLES` に `patch::META`、`Running` の `match` に `"patch" => { <PatchApp/> }`(plain は無し)。gallery の `Options::setup` で `patch::gpu::setup` も呼ぶ(`shader` と並べる)。
- `Meta` の `hooks` に `use_future` と `use_memo`、`elements` に `Canvas` / `Suspense` / `ComboBox` / `TextEdit` を入れる。gallery のタグ絞り込みから辿れるようにする。
- README の表に 1 行(live / source、plain は `–`)。
- snapshot は取らない(GPU の出力がプラットフォームで揺れる)。`shader` と同じ扱い。

## 8. 実装で判明した差分

(実装しながら書く。`ItemStyle` への絶対配置、ポート結線に足りなかったもの、`Cx::new` で降りる形の書き心地など、ライブラリ側の宿題はここに残す。core の変更が要ると判断した場合はこの PR では入れず、別 PR に切る。)
