# プラン: patch

タスク定義は [task.md](task.md)。設計の根拠は [docs/ARCHITECTURE.md](../../ARCHITECTURE.md)。[board](../board/plan.md) と 1 つの PR で、board を仕上げてからここに入る。実装中にここから外れる判断をした場合は本書を更新し、設計上の意味があれば ARCHITECTURE.md も更新する。

## 0. 全体

```
examples/patch/src/
  lib.rs      App と 3 ペイン。パッチキャンバスとノードもここ(gallery が見せるのはこれ)
  graph.rs    Graph / Node / Kind / Msg / reduce。純関数、ユニットテスト付き
  codegen.rs  Graph -> WGSL の文字列。naga での検証。純関数、ユニットテスト付き
  prelude.wgsl 生成コードが乗る土台(uniform、頂点シェーダ、色変換)。include_str!
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
    /// グラフ座標。`egui::Pos2` ではなく `[f32; 2]`(下記)
    pub pos: [f32; 2],
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
    /// プリセットで丸ごと差し替える(4.2)。両方の rev を今より先に進める
    Load(Graph),
}
```

`graph.rs` は `board.rs` と同じく egui を知らない。`pos` が `egui::Pos2` ではなく `[f32; 2]` なのはそのためで、`use_persisted` と `preset` の JSON が `emath` の serde feature(誰が有効にしているかは依存の都合で決まる)に依存しなくなるという実利もある。画面側で `egui::pos2(node.pos[0], node.pos[1])` に直す。

ノード名は生成される WGSL にコメントとして出る(`// level1`)ので、`Rename` は topology 側である。

**`topology_rev` と `param_rev` を分けるのがこの example の芯。** `reduce` は変更の種類でどちらを上げるか決める(`SetParam` と `MoveNode` と `Raise` は param 側、それ以外は topology 側)。スライダを動かしても WGSL が再生成されないことは、テスト P-5 で固定する。

`Output` は 1 つだけ。`AddNode` では作らせず、初期グラフが持つ。

## 2. WGSL の生成(`codegen.rs`)

```rust
pub struct Generated { pub wgsl: String, pub slots: Vec<NodeId>, pub hash: u64 }
pub fn generate(graph: &Graph) -> Result<Generated, GenError>;
pub fn pack_params(graph: &Graph, slots: &[NodeId]) -> [[f32; 4]; 32];
pub enum GenError { Cycle(NodeId), TooManyNodes, Wgsl(String) }
```

`hash` は生成した文字列そのもののハッシュで、pipeline を作り直すかどうかの判定に使う(3 章)。GPU 側で計算すると「同じ文字列か」を毎フレーム測ることになるので、作った側が 1 度だけ数える。

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

- prelude(uniform 宣言、fullscreen triangle の頂点シェーダ、`rgb2hsv` / `hsv2rgb`)は `include_str!("prelude.wgsl")` で、生成部分の**前**に置く。`fs_main` は prelude ではなく codegen が最後に出す(`n<output>(uv)` を呼ぶだけ)。前方参照を一切作らないための順番で、prelude に `fs_main` を置くと prelude が生成関数を先に呼ぶことになる。
- 循環はグラフ全体に対して検出する(`Output` から辿れる範囲だけではない)。繋がっていない場所の循環も同じ文言で報告される代わりに、「循環はどこにあってもエラー」と 1 行で言える。
- スロットは `Output` から辿れるノードに、生成順で 1 つずつ配る。パラメータを持たない `Grayscale` や `Output` にも配るのは、codegen と uniform の詰め方を「順番に 1 つずつ」で揃えるためである。
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
└ PatchProvider              Dispatch / Dnd / ポート位置を provide_context(board と同じ形)
  └ View row grow
    ├ Palette (w=150)        タイトル、undo / redo、ノード種のボタン。クリックで追加
    └ Suspense fallback      ★ プリセットを待つのはここから右だけ(4.2)
      └ Stage                プリセットを読む use_future と、下の 2 列
        ├ View column grow   ★ パッチキャンバス(下の 4.1)
        └ View column (w=300)
          ├ Preview          <Canvas> + wgpu callback。再生 / 一時停止
          └ Inspector        タブ: params / wgsl
```

`Stage` は `#[component]` だが、Ui を切り取らない(`cx.scope` は taffy モードでは `with_auto_id_prefix` だけで、ノードを増やさない)。`Suspense` は `shares_ui` なので、この 2 列はそのまま行のフレックス item になる。undo / redo をツールバーではなくパレットの頭に置いたのは、待たされる側(Suspense の中)に入れないためである。

- `Inspector` の `params` は選択ノードの `match kind` で別コンポーネント(`LevelParams` `MixParams` ..)。`wgsl` タブは生成されたコードとエラーをそのまま出す。**これは見せるための機能であり、テストが生成結果を UI から読む口でもある**(6 章)。
- 派生の連鎖:

```rust
let gen = use_memo(cx, (graph.topology_rev, epoch), || codegen::generate(&graph));   // Result
let program: State<Option<Program>> = ..;   // Ok の時だけ差し替え、Err なら直前を保つ
let params = use_memo(cx, (graph.param_rev, graph.topology_rev, epoch), || {
    codegen::pack_params(&graph, &program.slots)
});
```

  スライダを動かした時に走るのは 2 本目だけである。この 2 段構造を `lib.rs` の冒頭コメントで説明する。

  deps の `epoch` は undo / redo の回数である。**rev は undo で巻き戻る**ので、それだけを deps にすると「undo して別の編集をした」時に前と同じ rev が別の内容に付きうる。実際には 2 つのメッセージの間に必ず 1 フレーム描かれ、memo はその中間の rev を見て作り直すので事故にはならないが、それは deps の性質ではなくタイミングの話であり、1 回の reducer 訪問に 2 つのメッセージが入った瞬間に崩れる。epoch は決して戻らないので `(rev, epoch)` は単調で、この問いが立たない。board の `Board::rev` も同じ形をしている(8 章)。

  直前の成功結果を持つのは memo ではなく `use_state<Option<Program>>` である。memo は「deps が変わったら作り直す」だけで「前の値を残す」意味論を持たないので、`Err` の時に前の pipeline と前の WGSL を見せ続ける役は state が持つ(P-4)。書き込みは `hash` が変わった時だけなので、毎フレーム dirty にはならない。

### 4.1 パッチキャンバス

ノードの配置は絶対座標で、`ItemStyle` に絶対配置は無い。`escape-hatch` example の `Nested` と同じ形で降りる。

```rust
let (store, scope) = (cx.store, cx.scope_id());
// <View grow={1.0} h={0.0}> の中の {view(move |cx| cx.leaf_fill(&style, move |ui| { .. }))}
//   1. ui.allocate_exact_size(available, Sense::click_and_drag()) で領域と背景の応答
//   2. 背景のドラッグでパン(親の use_state<Vec2> に on_pan で返す)
//   3. ワイヤの置き場所を painter に 1 つ予約する(Shape::Noop)
//   4. ノードごとに:
//        ui.scope_builder(egui::UiBuilder::new().max_rect(node_rect), |ui| {
//            let mut cx = Cx::new(store, ui, scope);
//            cx.scope(node.id, |cx| rsx! { <NodeView ../> }.show(cx));   // ← これが key
//        });
//   5. ノードが登録したポート位置でワイヤを組み、予約した場所に painter.set する
```

- `cx.scope(node.id, ..)` が rsx! の `key={..}` に当たる。ここだけ手で書くので、**key の正体が「スコープ Id に混ぜるもの」であることがコードに出る**。コメントで board の `key=` と結び付ける。
- ノードの中は普通に `<View>`。Ui モード直下の `container` は新しい taffy ツリーを開くので、ヘッダ行(名前 + 折りたたみ)とパラメータの縦並びはフレックスで組める。
- ノードのローカル state: `collapsed` / `editing_name` / `draft_name` / `hovered_port`。**ノードを消しても残りのノードの状態が付いて回る**ことをテスト P-3 で固定する。
- ここでは `use_identity` は要らない。ノードのスコープは `キャンバス → node.id → NodeView` で、`graph.nodes` の中の位置が入らないためである(`Raise` で並べ替えても、隣のノードを消しても、そのノードのスロット Id は変わらない)。board の `<Card key={id}/>` が identity では足りなかったのは、カードのスコープに**列**が挟まっていたからで、ここは挟まっていない。この違いは lib.rs のコメントに書く。
- **ドラッグ中のオフセットはキャンバスが持つ**(`use_state<Option<(NodeId, Vec2)>>`)。ノード矩形はノードを描く**前**に決めないといけないので、位置に効く値だけはキャンバスの領分になる。ノードは掴んだこと・動かした量・離したことを `#[event]` で上げるだけで、`Msg::MoveNode` は離した時に 1 回だけ飛ぶ(ドラッグ中に毎フレーム飛ばすと undo が 1 ピクセルに 1 段になる)。
- **ポートの位置は毎フレームの登録制**にする。ノードのヘッダ行はフレックスなので、ポートの円の中心は描いてみないと分からない。`Rc<RefCell<HashMap<PortRef, Pos2>>>` を context で配り、ノードが自分のポート位置を書き込み、キャンバスが 5 で読む。`Handle` に dirty にしない書き込みが無いのは board 8.2 と同じ事情で、同じ回避をする。
- z 順は `graph.nodes` の順。掴んだら `Msg::Raise` で末尾へ。
- ズームは入れない(task.md の決めごと)。パンのみ。

### 4.2 プリセットと Suspense

```rust
let loaded = use_future(cx, (), || async { preset::parse(preset::STARTER) });
let Poll::Ready(graph0) = loaded else { return };   // 最も近い <Suspense> が fallback を描く
```

`STARTER` は `include_str!` の JSON なので待ち時間はほぼ無いが、`<Suspense>` は suspended から始まる(ARCHITECTURE 5.8)ので、fallback は必ず 1 パス以上描かれる。`<Suspense fallback={..}>` はキャンバスとプレビューだけを包み、パレット(undo / redo を含む)は待たせない。**画面の一部だけが待つ**ことの実演がここでの役目で、実アプリなら fetch になる旨をコメントに書く。

グラフそのもの(`use_persisted` + `use_undoable`)は `Suspense` の**上**にある。パレットが待たされずにノードを足せるためにはそこに無いといけないからで、読み込んだプリセットは `Stage` から `Msg::Load` として 1 度だけ流す(保存されたパッチが `Output` 1 つだけ = 手付かずの時に限る)。`Load` は他の編集と同じ 1 メッセージなので、undo すると空のパッチに戻る。

## 5. board からの再利用

board の `hooks.rs`(`use_dnd` / `use_undoable`)をこの example から使う。方法は 2 つあり、board が終わった時点で決める。

- (a) `examples/board` を `patch` の依存にし、`board::hooks` を `pub` にして使う。追加のクレートが要らない。
- (b) `examples/hooks`(仮)を新しいクレートに切り出し、board と patch が両方依存する。

**(a) を既定とする。** gallery の example どうしが依存し合うのは行儀が悪いが、切り出しは「同じ hook が 2 つの画面で使い回せた」という主張を薄める。どちらにせよ、hook の中身は両方の example で 1 つである。(b) に倒すのは、board 側の hook が patch のために形を歪められる場合だけ。

実装した結果は **(a)** で、`examples/patch/Cargo.toml` が `board = { path = "../board" }` を持ち、`use board::hooks::{Dnd, Undoable, use_dnd, use_undoable};` と書く。board 側は 1 文字も変えていない。

`use_dnd<P, T>` は**結線だけ**に使う(`P = T = PortRef { node, port }`:出力ポートを掴んで入力ポートに落とす)。ノードの移動には使わない。`Dnd` は「掴んだものを、登録された slot のどれかに落とす」道具で、落とし先が要らない移動は `Response::drag_delta` そのものだからである。掴んだ payload と落とし先の型が同じなのは、どちらもポートだからで、退化ではない。

`use_undoable` は `Graph` に対してそのまま使う。`use_identity` は使わない(4.1)。板の 3 つの hook のうち 2 つが、形を変えずに別の画面で動いたことになる。

## 6. テスト(`tests/patch.rs`)

kittest は headless(GPU 無し)で回す。`<Canvas>` に積む wgpu callback は描画されないだけで、UI の検証には影響しない(`shader` example と同じ)。

- **P-1** パレットからノードを追加するとキャンバスのノードが増え、インスペクタに出る。
- **P-2** 追加したノードを Output に繋ぐと、`wgsl` タブの本文にそのノードの関数と呼び出しが現れる。
- **P-3** ノードを 1 つ削除しても、残ったノードの折りたたみ状態と名前の下書きが付いて回る(board B-2 の性質を、より深いツリーで)。
- **P-4** 循環を作るとインスペクタにエラーが出て、`wgsl` タブは直前の内容のまま残る。
- **P-5(目玉)** パラメータのスライダを動かしても `wgsl` タブの本文が 1 文字も変わらない。ノードを繋ぎ替えると変わる。= 再コンパイルとパラメータ更新が分かれていることの証明。
- **P-6** プリセットの読み込みで `<Suspense>` の fallback が 1 度は出て、その後キャンバスにノードが並ぶ。`Harness` は構築時にアプリを落ち着くまで回すので、素直に書くと「見た時にはもう解決済み」になる。テスト側で (a) 構築の間はアプリを描かない、(b) 最初のフレームだけ `max_passes = 1` にする、の 2 つを入れて、**future を起動したパスそのもの**を見る(そのパスで future が Ready になることは無い)。
- **P-7** undo で 1 つ前のプログラムに戻り、undo の後の別の編集はちゃんと新しいプログラムになる(4 章の epoch)。
- **P-8** 3 列が窓に収まる(`<Canvas>` と `ScrollArea` の `grow` + `h={0}`。board 8.3 と同じ罠)。
- **P-9** `Shader` ノードの式を書き換えるとプログラムが変わり、壊れた式は naga の文言としてインスペクタに出て、絵は直前のプログラムのまま残る(P-4 の「循環」に対する「WGSL として不正」の側)。
- **P-10** 同じ種類のノードを 2 つ足しても hook の Id が衝突しない(`Store::collisions()` が空)。キャンバスが `cx.scope(node.id, ..)` を手で書いている以上、これは見張る価値がある。
- `codegen` のユニットテスト: 各 Kind 1 つずつ、`Transform` の入れ子、同じ出力を 2 入力に繋いだ DAG、循環の検出、32 ノード超え。生成物は naga を通す(これで「生成した WGSL が常に妥当」がテストで担保される)。
- `graph` のユニットテスト: `reduce` が `topology_rev` / `param_rev` のどちらを上げるかを、全 `Msg` について 1 行ずつ。

## 7. gallery / README

- `examples/gallery/Cargo.toml` に依存を足し、`EXAMPLES` に `patch::META`、`Running` の `match` に `"patch" => { <PatchApp/> }`(plain は無し)。gallery の `Options::setup` で `patch::gpu::setup` も呼ぶ(`shader` と並べる)。
- `Meta` の `hooks` に `use_future` と `use_memo`、`elements` に `Canvas` / `Suspense` / `ComboBox` / `TextEdit` を入れる。gallery のタグ絞り込みから辿れるようにする。
- README の表に 1 行(live / source、plain は `–`)。
- snapshot は取らない(GPU の出力がプラットフォームで揺れる)。`shader` と同じ扱い。

## 8. 実装で判明した差分

core(`react-egui` / `react-egui-macros`)にも `react-egui-elements` にも手を入れていない。以下は「書けなかったこと / どう回避したか / 足すとしたら何か」。board 8 章と重なるものは、重なったという事実の方が情報なので明示する。

### 8.1 絶対配置は escape hatch で足りた。`ItemStyle` に `position` は要らない

キャンバスは leaf 1 枚で、その中で `ui.scope_builder(UiBuilder::new().max_rect(node_rect), ..)` → `Cx::new(store, ui, scope)` → `cx.scope(node.id, ..)` と降りる。ノードの内側は普通の `<View>` で、Ui モード直下の `container` が新しい taffy ツリーを開くので、ヘッダ行もパラメータの縦並びもフレックスで書ける。`escape-hatch` の `Nested` と同じ形が、そのまま「絶対配置のレイヤ」として使えたことになる。

**ここでの `key` は `use_identity` を要らなくする。** ノードの hook のスコープ連鎖は `キャンバス → node.id → NodeView` で、`graph.nodes` の中の位置が一切入らない。`Raise` で並べ替えても隣を消してもスロット Id が変わらないので、board 8.1 の `use_identity` はここでは不要である(board でそれが要ったのは、カードのスコープに**列**が挟まっていたから)。「identity で state を持つ」ための道具は 2 つあり、**木の形が識別子を含んでいるならそれで足りる**、というのが 2 つの example を並べて分かったことである。

**逆に、ドラッグ中のオフセットだけはキャンバスの持ち物になる。** ノードを描く矩形はノードを描く前に決まっていなければならず、位置に効く値は「部品の中」に置けない。ノードはジェスチャを `#[event]` で上げ、キャンバスが位置を持ち、離した時に `Msg::MoveNode` を 1 回だけ送る(毎フレーム送ると undo が 1 ピクセルに 1 段になる)。**部品の state を全体が持つのが常に間違いというわけではなく、「描く前に要る値」は全体の側だ**という線が引けた。

### 8.2 `<View>` は矩形を返さない(board 8.4 の再確認)、ので毎フレームの登録制にした

ワイヤの端点はポートの円の中心で、円はヘッダ行のフレックスの中にある。`<View>` も `<Frame>` も `Response` を返さないので、位置は「描いたノードが自分で報告する」しかない。`Ports`(`Rc<RefCell<HashMap<PortRef, Pos2>>>`)を context で配り、ノードが `put`、キャンバスが読む。`Handle` に dirty にしない書き込みが無いのは board 8.2 のとおりで、同じ回避(`Handle::with` + 内部可変性)をした。**同じ穴に 2 つの example が落ちたので、`Handle::with_mut` は宿題として本物である。**

ワイヤはノードの**下**に描きたいが、位置はノードを描いた**後**にしか分からない。`let idx = painter.add(Shape::Noop);` で場所を予約し、ノードを描いてから `painter.set(idx, Shape::Vec(wires))` で埋めた。egui の painter がこれを持っていたので、レイヤを足す必要は無かった。

足すとしたら board 8.4 と同じ `<View>` の `#[event] on_rect`。それがあれば `Ports` は要らなくなる。

### 8.3 `leaf_fill` は「残り」ではなく「全部」(board 8.3 の再確認)

キャンバス(leaf_fill)も `ScrollArea` も、`grow={1.0}` だけでは窓の高さを丸ごと要求して下の行を押し出す。`grow={1.0} h={0.0}`(`shader` example の `<Canvas>` と同じ)で「余りだけ取る」になる。列の高さが確定しているのは、行に `h="100%"` があり、ルートが `min_h: 100%` だからで、この連鎖のどこかが auto になると効かなくなる。P-8 がこれを見張っている。

### 8.4 `bind` を持つ要素は「今の値を報告する」ことができない

`<TextEdit>` の `bind` が唯一の `&mut String` を握るので、同じ要素のハンドラは同じ文字列を読めない(ARCHITECTURE 3.7 の 2 つ目)。`on_change` はペイロードを持たず、`on_submit`(Enter)だけが文字列を運ぶ。`Shader` ノードの式は「1 文字打つたびに再生成して naga に通す」ことが見せ場なので、Enter では遅い。そこで `SourceEdit` を leaf 1 枚で自分で書き、毎フレームのコピーに書かせて `changed()` の時だけ `on_change.emit(text)` した。

足すとしたら `TextEdit` の `on_change` にペイロード(新しい文字列)を持たせること。`bind` が `&mut` を握っているので、要素の側でクローンを 1 つ作れば済む。

### 8.5 描いた円とグリフのボタンは、名前を言わないと存在しない

ポートは `ui.allocate_exact_size` + painter で、ノードの畳み / 名前 / 削除は "-" "name" "x" のボタンである。前者は accessibility ツリーに何も残さず、後者は 8 個のノードに同じ "x" が並ぶ。`Response::widget_info(|| WidgetInfo::labeled(..))` で「`shader1 out`」「`delete level1`」と名前を付けた。スクリーンリーダのための話であって、テストのための話ではない(テストが書きやすくなったのは結果である)。

ついでに分かったこと: `egui::Slider` は accessibility ノードを **2 つ**作る(スライダ本体と、値を表示する drag value)。どちらもラベルが同じなので、`get_by_label` は必ず 2 件見つける。kittest では `get_by_role_and_label(Role::Slider, ..)` で引く。

### 8.6 `<Suspense>` は `Harness` の構築後には見られない

`Harness::from_builder` は最後に `run_ok()` を呼び、アプリが落ち着くまで回す。プリセットの future はスレッド 1 本と `include_str!` の parse なのでその間に終わり、テストが最初に覗いた時には境界はもう解決している。P-6 はそこで (a) 「まだ描かない」フラグを持つアプリで構築し、(b) `max_passes = 1` にしてから 1 パスだけ回す、という 2 段構えにした。**future はそれを起動したパスで Ready になることはない**ので、これは時間ではなく因果に依る。

これはライブラリの穴ではなく、テストの書き方の知見である。`<Suspense>` を持つ example のテストはこの形になる。

### 8.7 naga は wgpu 経由では当てにできない

`wgpu::naga` の re-export は `cfg(wgpu_core)` か `cfg(naga)` の下にあり、バックエンドの構成で消える(wasm の WebGPU だけの構成など)。生成した WGSL を CPU で検証するのはこの example の芯なので、workspace に `naga = { version = "30.0", features = ["wgsl-in"] }`(wgpu 30 が使うのと同じ版)を足した。`codegen.rs` は naga だけを知り、wgpu も egui も知らない。

`ParseError::emit_to_string` / `WithSpan::emit_to_string` がそのまま UI の文言になる(行番号と抜粋つき)ので、エラー整形は 1 行も書いていない。

### 8.8 細かいもの

- ノード種ごとのコンポーネントは、ノードの中とインスペクタで**同じものを 2 度使う**形にした(`wide` prop 1 つで drag value とスライダを切り替える)。「`match kind` で別コンポーネント」と「狭い所と広い所で見た目を変える」は両立する。
- WGSL は前方参照を作らない順(prelude → ノード関数 → `fs_main`)で連結する。`fs_main` は生成側が出す。
- `Msg::Load(Box<Graph>)` は `Box` に入れてある。メッセージ 1 個が `Graph` 1 個分の大きさになると、`Vec<Msg>` のキューが他の全メッセージの分まで太る。
- `graph.rs` は egui を知らない(`pos: [f32; 2]`)。board の `board.rs` と同じ方針で、`use_persisted` と preset の JSON が emath の serde feature に依存しないという実利もある。
