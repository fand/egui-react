# プラン: a11y(web のアクセシビリティ)

タスク定義は [task.md](task.md)。設計の根拠は [docs/ARCHITECTURE.md](../../ARCHITECTURE.md)。本書は調査の結果と、そこから決めた作業手順を定める。実装中にここから外れる判断をした場合は本書を更新し、設計上の意味があれば ARCHITECTURE.md も更新する。

調査は 2026-09 に行った。対象は egui / eframe 0.36.1、accesskit 0.24.1、accesskit_consumer 0.38.0(このリポジトリの `Cargo.lock` が引いている版。上流の最新は accesskit 0.25.0 / consumer 0.39.0、2026-08-29 リリース)。

## 0. 全体

**1 PR では終わらない。main に入るものと、`a11y-spike` ブランチに置くものに割る。** task.md の決めごと「react-egui の main に fork した eframe を依存として入れない」を守るためで、境界は「crates.io の依存だけで完結するか」に置く。

**割り方は着手後に変わった。** 当初は手順 6 以降をまるごと `a11y-spike` に置く前提だったが、1.2 で「`TreeUpdate` を受け取るのに eframe の fork は要らない」(2.2 の案 C)ことが分かった。**手順 6〜10 は crates.io の依存だけで閉じるので main に入れる。** fork が要るのは手順 11(F2)だけで、そこだけが spike に残る。

| | 場所 | 内容 |
|---|---|---|
| PR #4(済) | main | 本書、ARCHITECTURE 1 章 / 8 章の一文、README の一文、`react-egui-elements` のラベル固め(`Image` の `alt`、`Button` の `label`)、名前の無いウィジェットを検出する kittest |
| PR #5(この続き) | main | `accesskit-web`(adapter crate)、`react_egui_app::a11y::WebA11y`(egui の plugin)、gallery での有効化、DOM → `ActionRequest`、フォーカス F1 |
| 続き | `a11y-spike` ブランチ | 手順 11 だけ: fork した eframe で `has_focus` を緩める F2 |
| その後 | 上流 | AccessKit へ web adapter の PR、eframe / egui へ差し込み口の issue |

割り方の根拠は 2 つ。

1. **ラベル固めは上流と無関係に効く。** native の VoiceOver / NVDA / Orca には今日から効き、web adapter が入った日にもそのまま効く。むしろ adapter が動いた瞬間に「名前の無いボタン」が全部露見するので、先に潰しておく方が試作が読みやすい。
2. **fork が要るかどうかは 1 点に絞れた。** 当初は「試作は crates.io の依存だけでは閉じない可能性が残る」と書いたが、閉じないのは**フォーカスの往復だけ**である(2.3)。ツリーを受け取る側も `ActionRequest` を返す側も egui の `Plugin` で足りる。だから spike に残すのは F2 の 1 コミットで済む。

## 1. 調査結果

### 1.1 AccessKit の web adapter: 試作が 1 本あり、フォーカスで止まっている

task.md の背景表は「AccessKit に web adapter が存在しない」と書いたが、**正確には「リリースされていないが、ブランチに 500 行ほどの試作がある」**。

- 議論: [AccessKit/accesskit discussions#514 "Web Adapter"](https://github.com/AccessKit/accesskit/discussions/514)(2025-02)。外部の人(floers)が「web adapter を書きたいが重複作業か」と聞き、メンテナ(DataTriny)が「[`web-basics` ブランチ](https://github.com/AccessKit/accesskit/tree/web-basics)にすでにある。かなり動くが数ヶ月触っていない。引き継ぐ気はあるか」と答えた。
- **その後が本題。** floers が 2025-03 にこのブランチを土台に **Slint の web デモ**を作り([floers/accesskit の `web-basics`](https://github.com/floers/accesskit/tree/web-basics)、`examples/slint-web`)、DOM のボタンを押すと Slint 側が反応するところまで動かした。**逆方向の作り方も同じ結論に達している**: 「`action_handler` は実装するものではなく、**使って** action を送るものだった。DOM のボタンに JS の `on_click` を付けて `action_handler` 経由で投げたら動いた」。
- **そしてメンテナの検証(2025-03-09)がこう終わっている。** 「試したが **as-is ではスクリーンリーダーで使えない。フォーカスの移動が信頼できない**(Slint 側の問題かもしれないが)。デモとしては良い」。以降このスレッドは止まっている。**1.3 で我々が独立に見つけた壁と同じ場所**である。
- ブランチの最終コミットは `51dbebd`(2024-07-15、"Set application role on the root node")。中身は `platforms/web/` に `Cargo.toml`(crate 名 `accesskit_web` 0.1.0)、`src/lib.rs` / `adapter.rs`(6.6 KB)/ `node.rs`(11.3 KB)/ `filters.rs`。AccessKit の README の「Planned adapters」に web が挙がっている。
- **crates.io に `accesskit_web` は無い**(`https://crates.io/api/v1/crates/accesskit_web` が 404)。リリースの一覧にも web は無く、直近の 2026-08-29 のリリース波は common / consumer / windows / macos / unix / android / **ios** / winit で、iOS adapter([issue#565](https://github.com/AccessKit/accesskit/issues/565))は `accesskit_ios` 0.2.0 として出ている。web だけが空いている。
- リポジトリ全体を「web」「DOM」「wasm」「browser」で検索した限り、他に web adapter の議論は無い。DOM 関連で出てくるのは[issue#705 "Add a property to allow exposing DOM ID"](https://github.com/AccessKit/accesskit/issues/705) → [PR#776 `html_id` node property](https://github.com/AccessKit/accesskit/pull/776)(2026-08-25 merge)だが、これは **Servo が AccessKit を「web コンテンツ → OS」の向きで使う**([servo/servo#4344](https://github.com/servo/servo/issues/4344))ための話で、向きが逆である。

`web-basics` の中身を読むと、**骨格はそのまま使えるが、動くところまでは行っていない**。

| ある | 無い |
|---|---|
| `Adapter::new(parent_id, ActivationHandler, ActionHandler)`、`update_if_active(impl FnOnce() -> TreeUpdate)`、`update_host_focus_state(bool)` | **座標**。ノードは入れ子の `<div>` を作るだけで、`bounding_box()` を読んでおらず、`position: absolute` も CSS も一切無い。canvas に重ならない |
| `Tree` / `TreeChangeHandler`(`node_added` / `node_updated` / `focus_moved` / `node_removed`)で差分だけ DOM に反映 | **DOM → egui の経路**。`action_handler` を持つだけで、**一度も呼んでいない**。click も keydown も listener が無い |
| `Role` → ARIA role の対応表(約 150 行、`CheckBox`→`checkbox`、`TextInput`→`textbox`、`Slider`→`slider`、…) | Tab で辿れる順序。`tabindex` は focusable なら `"-1"` 固定 |
| `aria-label` / `aria-checked` / `aria-valuemin` / `aria-valuemax` / `aria-valuenow` / `aria-valuetext`、`Role::Label` だけ `textContent` | `aria-expanded` / `aria-selected` / live region / テキスト選択 / スクロール |
| `focus_moved` で `element.focus()` / `blur()` | DOM 側でフォーカスが動いた時に egui へ返す経路 |

さらに **API が 2 年ぶんずれている**。`Node::name()` は `label()` に、`is_focusable()` は `is_focusable(&parent_filter)` に変わり、accesskit 0.24 で複数ツリー対応([PR#655](https://github.com/AccessKit/accesskit/pull/655))が入って `TreeUpdate` に `tree_id`、`ActionRequest` に `target_tree` が増えた。ブランチが依存しているのは accesskit 0.16 / consumer 0.24 なので、そのままはビルドしない。

**結論: ゼロからではなく、この 500 行を現行 API に移植し、抜けている「座標」「DOM → egui」「Tab 順」を足すのが最短路。** 上流に持ち込むときも「あなたのブランチを起こしました」という形になるので、話が早い。そして **web adapter が止まっている理由は「フォーカス」の 1 点**だとメンテナ自身が書いているので、試作が出す価値のある答えもそこにある(1.3、2.3)。「ツリーは映る。フォーカスがこの形なら通る/通らない」を実測して返すのが、この試作の一番の成果になる。

なお、`Cargo.lock` には consumer が 0.35 / 0.36 / 0.38 の 3 つ入っている(kittest 0.4 が 0.35、accesskit_windows が 0.35、atspi が 0.36、accesskit_macos が 0.38)。すべて accesskit 0.24.1 に対して動いているので、**試作の adapter も `accesskit = "0.24.1"` + `accesskit_consumer = "0.38"` で組める**(macos adapter と同じ組み合わせ)。egui が握る `TreeUpdate` と型が一致することが条件なので、`accesskit` の版だけは egui に合わせる。

### 1.2 eframe 0.36 web: ツリーは確かに捨てられているが、fork 無しで拾える

`crates/eframe/src/web/app_runner.rs` の 394 行目、`handle_platform_output` の分解パターンに `accesskit_update: _, // not currently implemented` がある(手元の `~/.cargo/registry/.../eframe-0.36.1/src/web/app_runner.rs:394`)。ここは task.md のとおり。ただし調べた結果、**周辺の前提が 3 つ違っていた。**

**(a) `accesskit` は egui の feature ではない。** eframe の `accesskit` feature は `["egui-winit/accesskit"]` だけで、native 専用である。egui 本体は `accesskit = "0.24.1"` を**無条件の依存**として持ち、`Context::enable_accesskit()` / `disable_accesskit()` / `accesskit_node_builder()` はいつでも呼べる(`egui-0.36.1/src/context.rs:3698`)。**wasm でも `ctx.enable_accesskit()` を呼べばその場でツリーが作られ始める。** 呼ぶ主体が居ないだけで、機能は生きている。呼ぶのはアプリでよく、`react_egui_app::Options::setup` の `&CreationContext` から `cc.egui_ctx.enable_accesskit()` と書ける。

**(b) egui 0.36 には `Plugin` trait があり、`FullOutput` を横から掴める。** `egui::plugin::Plugin`(`egui-0.36.1/src/plugin.rs`)は次を持つ。

```rust
pub trait Plugin: Send + Sync + std::any::Any + 'static {
    fn debug_name(&self) -> &'static str;
    fn setup(&mut self, ctx: &Context) {}
    fn on_begin_pass(&mut self, ui: &mut Ui) {}
    fn on_end_pass(&mut self, ui: &mut Ui) {}
    fn input_hook(&mut self, ctx: &Context, input: &mut RawInput) {}
    fn output_hook(&mut self, ctx: &Context, output: &mut FullOutput) {}   // ← ここ
}
```

`Context::end_pass()` の最後で `plugins.on_output(self, &mut output)` が呼ばれ(`context.rs:2440` 付近)、その時点で `platform_output.accesskit_update` はすでに埋まっている(埋めるのは `ContextImpl::end_pass`、`context.rs:2692` 付近)。登録は `Context::add_plugin(impl Plugin)` で public。

つまり **`TreeUpdate` を受け取るのに eframe を fork する必要は無い。** 逆向き(`ActionRequest` の注入)も `Plugin::input_hook(&mut RawInput)` で足りる。eframe 側にも `App::raw_input_hook(&Context, &mut RawInput)` という public な穴がある(`eframe-0.36.1/src/epi.rs:279`)が、plugin の方が eframe に依存せず、kittest でもそのまま動くので好ましい。

注意が 2 つ。

- `Plugin` は `Send + Sync` を要求するが、`web_sys::HtmlElement` などは `!Send`。**DOM 側の状態は `thread_local!` のレジストリに置き、plugin 構造体は整数のキーだけを持つ**形にする(wasm は単スレッドなので `unsafe impl Send` を書く必要も無い)。
- `output_hook` は**パスごとに**呼ばれる。egui は `request_discard` されたパスをやり直すので(`context.rs:833` の `loop`)、react-egui では taffy が普通に 2 パス回る(ARCHITECTURE 5.3)。`output.platform_output.requested_discard()` が立っているパスの `TreeUpdate` は**捨てる**。捨てたパスのツリーを DOM に流すと、レイアウトが決まる前の座標が一瞬出る。

**(c) web 側に流用できる「隠し DOM」は screen reader 経路ではなく text agent。** `web_screen_reader` feature(既定 on)の実体は `web/screen_reader.rs` の `speak(text)` だけで、`speechSynthesis` に `platform_output.events_description()` の 1 行を投げる。ツリーもフォーカスも無い。流用価値は無い。

代わりに参考になり、かつ**衝突する**のが `web/text_agent.rs` である。IME とモバイルキーボードのために、透明な `<input>` を canvas の兄弟として `position: absolute` で canvas の左上に置いている。DOM ミラーがやりたいことの縮小版がすでにそこにある。

### 1.3 eframe web のフォーカスは canvas に固定されている(ここが唯一の本当の壁)

`AppRunner::has_focus()` は「canvas か text agent が `document.activeElement` か」で、それ以外の要素にフォーカスがあると **`false`** を返す(`app_runner.rs:227`、`web/mod.rs:77` の `has_focus`)。そして `logic()` の頭で `update_focus()` が `input.raw.focused = false` を立てる。さらに `handle_platform_output` は、アプリにフォーカスがある間、毎フレーム `focus_without_scroll(self.canvas())` を呼んで canvas にフォーカスを引き戻す。

つまり **ミラーの `<div>` に `element.focus()` すると、egui は「自分はフォーカスを失った」と判断してキー入力を受け付けなくなる。** `web-basics` の adapter がやっている `focus_moved` → `element.focus()` を eframe の上でそのまま動かすと、ここで壊れる。

逃げ道は 2 つある。

| | やり方 | 代償 |
|---|---|---|
| F1 | DOM フォーカスは canvas に置いたまま、canvas に `aria-activedescendant` で「今どのノードに居るか」を伝える | fork 不要。ただし `aria-activedescendant` の支援技術対応は role が限られ、VoiceOver では読み上げが落ちることが知られている。Tab キーは canvas 1 個しか止まらない |
| F2 | ミラーの要素に本物のフォーカスを渡し、eframe の `has_focus` を「canvas の親コンテナの中に activeElement があるか」に緩める | **eframe の fork が要る**(数行)。支援技術の挙動は素直 |

**Flutter は F2 側(本物の DOM フォーカス + `tabindex="0"`)を採っていて、`aria-activedescendant` は使っていない**(1.4)。前例に従うなら F2 が本命で、F1 は「fork せずにどこまで行けるか」を測るための当て馬である。**まず F1 で組んで VoiceOver で確かめ、駄目なら F2 に落ちる。** そして F2 が必要だったことがそのまま「eframe への上流提案」の中身になる。上流に出す形は「`WebOptions` に `accesskit` の口を開ける」ではなく、**eframe が adapter を持つ**のが本筋である。eframe は canvas も text agent もフォーカスも持っているので、外から差し込むより eframe の中に置いた方が短く済む。差し込み口を提案する場合でも、`has_focus` の緩和が一緒に要ることを添える。

### 1.4 Flutter web の semantics 層

同じ方式(canvas 描画 + DOM の鏡写し)を本番で回している唯一の前例。engine 統合後の現在地は [`engine/src/flutter/lib/web_ui/lib/src/engine/semantics/`](https://github.com/flutter/flutter/tree/master/engine/src/flutter/lib/web_ui/lib/src/engine/semantics)。**28 ファイル、約 280 KB の Dart**(`semantics.dart` だけで 3,490 行)。role ごとにファイルが割れている: `checkable.dart` / `focusable.dart` / `incrementable.dart` / `label_and_value.dart`(24 KB)/ `link.dart` / `live_region.dart` / `menus.dart` / `route.dart` / `scrollable.dart` / `table.dart` / `tabs.dart` / `tappable.dart` / `text_field.dart`(17 KB)/ `platform_view.dart` など。task.md の見積り「実用は数千行」はこの規模のことで、当たっている。

**DOM の形**(`semantics.dart` と [`view_embedder/dom_manager.dart`](https://github.com/flutter/flutter/blob/master/engine/src/flutter/lib/web_ui/lib/src/engine/view_embedder/dom_manager.dart))。

```
<flutter-view>
  <flt-glass-pane> -> #shadow-root { <flt-semantics-placeholder>, <flt-scene-host><flt-scene>(canvas) }
  <flt-text-editing-host>
  <flt-semantics-host>     ← 最後に append する。「ヒットテストの順では最初に来る必要があるため」
```

- **ミラーは canvas の兄弟で、DOM の最後**。eframe の text agent が canvas の兄弟に置かれているのと同じ位置取りで、`<flt-text-editing-host>` が別にあるのもそっくりである。
- ノード 1 つに `<flt-semantics id="flt-semantic-node-N">` 1 つ(`SemanticRole.createElement`、724 行)。`position: absolute` + `overflow: visible`(`_initElement`、727 行)、大きさは矩形の px、位置は `transform-origin: 0 0 0` + CSS `transform` の行列。子は親の矩形とスクロール量を引いた相対座標にする(`recomputePositionAndSize` / `recomputeChildrenAdjustment`)。
- **ホストが DPR を吸収する。** `<flt-semantics-host>` に `position:absolute; left:0; top:0; transform-origin:0 0 0; transform: scale(1/devicePixelRatio)`。コメントいわく「フレームワークは semantics を物理 px で出すが、CSS は論理 px を使う」。**2.1 でも同じ手を使う**(egui のツリーも root に `Affine::scale(pixels_per_point)` が乗っているので、ホスト 1 枚で割り戻せて、ノードごとの割り算が要らない)。
- **透明にする方法が学べる。** **root ノードだけ**に `filter: opacity(0%)` と `color: rgba(0,0,0,0)`。コメントに理由が書いてある: 「`opacity` 属性ではなく `filter` を使う。`filter` の方が強く、iOS のスライダーの thumb / track のように `opacity` が効かない要素があるため」「`visibility: hidden` / `display: none` ではなく透明化を使う。スクリーンリーダーがこれらの要素を無視しないようにするため」。**2.1 のホストの CSS はこれに倣う。**
- **本物の要素を使うところは使う。** link → `<a href>`、heading → `<h1>`〜`<h6>`(margin / padding 0、font-size 10px)、form → `<form>`、テキスト欄 → 子の `<input>` / `<textarea>`、スライダー → 子の `<input type=range role=slider>`。一方 **checkbox / radio / switch は本物の `<input>` にせず** `role` + `aria-checked` で作る。「全部 div」でも「全部本物」でもない。
- **`role="application"` はどこにも無い**(リポジトリ全体で 0 件)。`web-basics` が root に付けていたのとは逆の判断である。
- ラベルの出し方が 3 通りある(`label_and_value.dart` の `LabelRepresentation`)。`ariaLabel`(最も安いが「大半の web クローラと Windows の JAWS に効かない」)/ `domText`(テキストノード。button / link / heading)/ `sizedSpan`(CSS transform で実寸に伸ばした `<span>`。スクリーンリーダーのフォーカス枠がウィジェットの矩形と一致する。最も高い)。**`aria-label` 一本槍では駄目**というのがここの教訓。
- `pointer-events` は 3 段階(`acceptsPointerEvents`、684 行)。`all` = 対話的か `SemanticsHitTestBehavior.opaque` / `none` = `transparent` か子を持つコンテナ / `auto` = `defer` の非対話的な葉(重なりの解決をブラウザの z-index に任せる)。子が 2 つ以上あるときはヒットテスト順を z-index の反転で作る(DOM の並び順は読み上げの順に使うので、2 つの順序を別々に持っている)。
- 読み上げの通知は `aria-live` をノードに付けず、`document.body` 直下の `<flt-announcement-host>`(polite / assertive)に集約する。egui#2647(live region)を web でやるときに要る形。
- デバッグ時は `debugShowSemanticsNodes` で緑の `outline` を引く(`border` はレイアウトを動かすので使わない)。試作でも同じ仕掛けを最初に入れると早い。

**フォーカスとイベントの往復**(`focusable.dart` / `tappable.dart` / `pointer_binding.dart`)。ここが 1.1 の「フォーカスが信頼できない」の実際の対処なので、そのまま真似する価値がある。

- **framework → DOM**: `changeFocus()` は `focusWithoutScroll()` を**更新後のコールバックまで遅らせる**。そして **`blur()` は決して呼ばない**(「要素を blur するのは非常に間違えやすい」)。同じ値の再設定も `_lastSetValue` で握り潰す。**`web-basics` の `focus_moved` は `blur()` を呼んでいる**(1.1 の表)ので、ここは移植時に落とす。
- **DOM → framework**: `tabindex="0"` + `focus` / `blur` の listener で `SemanticsAction.focus` を送るが、**直前に自分が要求したフォーカスなら送らない**(`_lastEvent == requestedFocus`)。エコーの無限ループ止めである。
- **クリック**: `ClickDebouncer` が `pointerdown` からイベントを溜め、200ms で流すか `click` が来たら `tap` に変える(「スクリーンリーダーは 200ms よりずっと速くクリックを合成する」)。50ms 以内に流した直後の `click` は捨てる。**合成クリックと本物のポインタ操作を見分けるための時間窓**で、これが無いと二重発火する(#130162)。
- **スクロール**: `role=group` + DOM の `scroll` イベント → `SemanticsAction.scrollToOffset`。オーバーフローする子を 1 枚入れてスクロール範囲を偽装する。
- **ルート遷移**: 「web のスクリーンリーダーはやってくれない」ので、engine が自分で最初の focusable な子孫にフォーカスする。

**有効化**(`semantics_helper.dart`)。既定では semantics 層を作らず、支援技術が居ることを検出してから作る。作りっぱなしは重いため。

- デスクトップ(`DesktopSemanticsEnabler`): `<flt-semantics-placeholder role="button" aria-live="polite" tabindex="0" aria-label="Enable accessibility">` を `position:absolute; left:-1px; top:-1px; width:1px; height:1px` で窓の外に置き、**クリックされたら** semantics を有効にする。「Tab で入っただけでは有効にしない。実際にクリックさせる」とコメントにある。
- モバイル(`MobileSemanticsEnabler`): 同じ placeholder を画面全面に広げ、**クリック座標が placeholder のちょうど真ん中なら「スクリーンリーダーが送ったクリックだ」と解釈する**(331 行)というヒューリスティックで有効化する。
- **placeholder は 2026 現在も健在**で、既定 off のままである([docs.flutter.dev の web accessibility](https://docs.flutter.dev/ui/accessibility/web-accessibility): 「性能上の理由で web のアクセシビリティは既定では on になっていない。有効にするにはユーザーが `aria-label="Enable accessibility"` の見えないボタンを押す必要がある」)。スクリーンリーダーの自動検出は**入っていない**。アプリから明示的に有効化する `SemanticsBinding.instance.ensureSemantics()` が逃げ道。
- Flutter 3.32(2025-05)で semantics の構築が約 80% 速くなり web のフレーム時間が約 30% 縮んだが、**既定 on にはまだ届いていない**。「ミラーは重い」は今も事実である。

**既知の弱点。**「Flutter web も解けていない」の中身は、そのまま我々が踏む穴でもある。

| | 内容 |
|---|---|
| フォーカスリングが見えない | ミラーが透明なので、フォーカスされているノードが視覚的に分からない([#186044](https://github.com/flutter/flutter/issues/186044)、open) |
| ミラーがマウスを食う | `ensureSemantics()` を呼ぶと `GestureDetector` の `Tap*Details` が壊れる([#188859](https://github.com/flutter/flutter/issues/188859))、モバイル web で semantics の onTap が背後の要素も叩く([#160560](https://github.com/flutter/flutter/issues/160560)) |
| ブラウザがノードを勝手に潰す | role の無い素のテキストノードを Safari が暗黙にマージする([#166787](https://github.com/flutter/flutter/issues/166787)) |
| Tab の順序 | iframe 埋め込みで自然な順にならない([#162871](https://github.com/flutter/flutter/issues/162871)) |
| テキスト欄 | `excludeSemantics` が `<input>` の `aria-label` を壊す([#172206](https://github.com/flutter/flutter/issues/172206))。TextField 周りが 17 KB ある理由 |
| ページ内検索 | Ctrl+F で本文を探せない([#65504](https://github.com/flutter/flutter/issues/65504)、2020 年から open)。原因が明快で、**「遅延描画のため engine は今見えている一部の UI しか知らない」「レイヤやピクチャからウィジェットへ戻る線が無い」**。egui も同じ(むしろ immediate mode なので徹底している) |
| 翻訳 | ページ翻訳が効かない([#131984](https://github.com/flutter/flutter/issues/131984)) |
| SEO | インデックスされない([#46789](https://github.com/flutter/flutter/issues/46789)、2020 年から open)。a11y が前提条件として挙げられている |
| 性能 | ミラーの構築とスクロール時の再配置が重い(#163204、#159358)。3.32 の改善後も既定 on にできていない |

ページ内検索 / 翻訳 / SEO は task.md が最初から「含まない」に置いたもので、その判断は正しかったことが確認できた。**「有効にするとアプリの挙動が壊れる」種類のバグが多いこと**も、この方式の性格として覚えておく価値がある(タップの二重発火 #147050 / #153924、重なった要素を突き抜ける #163576、入力できない #129324、ダイアログが勝手に閉じる #149001)。

### 1.5 他の canvas 描画 UI

| | a11y の扱い |
|---|---|
| Google Docs(2021 に DOM 描画から canvas に[移行](https://workspaceupdates.googleblog.com/2021/05/Google-Docs-Canvas-Based-Rendering-Update.html)) | canvas の兄弟に**見えない SVG のオーバーレイ**を置き、`<rect aria-label="..." x y width height transform>` を文字の上に並べる(いわゆる "annotated canvas")。既定 off で、許可された拡張が `window['_docs_annotate_canvas_by_ext']` を立てたときだけ出る([Chromium の `gdocs_script.js`](https://chromium.googlesource.com/chromium/src/+/main/chrome/browser/resources/chromeos/accessibility/common/gdocs_script.js))。サードパーティのスクリーンリーダー向けには**別の画面外 DOM** をユーザー設定で出す。**形は違うが「canvas + 座標付きの見えない DOM」という骨格は Flutter と同じ** |
| makepad | web ランナーの `CxOsOp::AccessibilityUpdate(_) => {}` が**空**(`platform/src/os/web/web.rs`)。[makepad#196 "On Accessibility"](https://github.com/makepad/makepad/issues/196) が 2023 年から open |
| Bevy | `bevy_a11y` が AccessKit ツリーを作るところまでは web でも動くが、渡す先の `accesskit_winit` が **wasm32 では何もしない adapter** なので web には出ない。まさに egui と同じ形 |
| Slint | 公式ドキュメントが「web ではスクリーンリーダーなどのアクセシビリティ機能は使えない」と[明記](https://docs.slint.dev/latest/docs/slint/guide/platforms/web/)。1.1 の floers の試作は Slint で作られたもので、**この穴を埋めようとして止まった** |
| iced | web の a11y は無い。[iced#552](https://github.com/iced-rs/iced/issues/552) が 2020 年から open で、途中の PR も native 向け |
| Dioxus | web で本物の DOM を描くのでこの問題自体が無い(task.md が「それをやるなら Dioxus」と書いたとおり) |
| HTML の canvas fallback content | `<canvas>` の子要素を代替として読ませる仕様([HTML Standard 4.12.5](https://html.spec.whatwg.org/multipage/canvas.html#the-canvas-element))と `drawFocusIfNeeded()`([4.12.5.1.14](https://html.spec.whatwg.org/multipage/canvas.html#drawing-focus-rings-and-scrolling-paths-into-view))。**足りない**。fallback 要素はボックスを持たないので**位置が無く**、拡大鏡や点字ディスプレイのルーティングが働かない。座標を扱うはずだった `addHitRegion` は 2016 年に、`scrollPathIntoView` は 2024 年に「どのブラウザも実装しなかった」として仕様から削除された。Chromium の canvas 担当者自身が [whatwg/html#7490](https://github.com/whatwg/html/issues/7490#issuecomment-1039495724) で「canvas で対話的な UI を描くアプリの多くは(Google Sheets のセル格子など)**fallback ではない見えない DOM 要素**でアクセシビリティを実装している」と書いている。MDN も「canvas の内容はアクセシビリティツールに公開されない。一般に canvas は避けるべき」と書く。将来の解は [WICG の HTML-in-Canvas](https://github.com/WICG/html-in-canvas) |

**結論: 前例は Flutter web と Google Docs の 2 つで、どちらも「canvas + 座標を持つ見えない DOM」に行き着いている。標準の canvas fallback は当てにならないことも、仕様の削除履歴と実装者の発言で裏が取れた。** Rust 側は egui / Bevy / Slint / iced / makepad が揃って同じ場所で止まっており(AccessKit ツリーは作れる、web に出す adapter が無い)、**web adapter が 1 つ入れば全部が一度に動く**。上流に出す価値はそこにある。

### 1.6 egui / eframe 側の関連 issue

| | 内容 |
|---|---|
| [emilk/egui#167](https://github.com/emilk/egui/issues/167) | 元の a11y 要望。closed。AccessKit 導入([PR#2294](https://github.com/emilk/egui/pull/2294))で閉じた |
| [emilk/egui#2391](https://github.com/emilk/egui/issues/2391) | README の a11y の記述を直す。「web を含む AccessKit 未対応のプラットフォームには実験的な内蔵スクリーンリーダーがある」という文面が提案され、closed |
| [emilk/egui#2647](https://github.com/emilk/egui/issues/2647) | live region を出したい。**open**。web でも native でも要る |
| [emilk/egui#7679](https://github.com/emilk/egui/pull/7679) | アプリが `Ui` の下に自前の AccessKit サブツリーを組めるようにする RFC。closed / 未 merge |
| [emilk/egui#8410](https://github.com/emilk/egui/pull/8410) | root 以外の viewport にも adapter を付ける。open |

web の a11y そのものを扱う issue は **egui にも eframe にも無い**。`accesskit_update: _` のコメントが唯一の記録である。上流に出す issue は新規になる。

## 2. 設計

### 2.1 `accesskit_web`(adapter crate、egui 非依存)

task.md の決めごとどおり、AccessKit の `TreeUpdate` しか見ない。egui も eframe も知らない。`web-basics` の構成をそのまま引き継ぐ。

```
accesskit_web/
  src/lib.rs      pub use adapter::Adapter
  src/adapter.rs  Adapter: State { Pending | Active { tree, host, elements } }
  src/node.rs     NodeWrapper: Role -> ARIA role、属性、座標
  src/filters.rs  accesskit_consumer::common_filter
```

```rust
pub struct Adapter { /* .. */ }

impl Adapter {
    /// `parent` の下に隠しホストを作る。canvas と同じ containing block に置く。
    pub fn new(
        parent: &web_sys::Element,
        activation_handler: impl ActivationHandler,
        action_handler: impl 'static + ActionHandler,
    ) -> Self;

    /// 1 フレーム 1 回。捨てられたパスでは呼ばない。
    pub fn update_if_active(&mut self, update: impl FnOnce() -> TreeUpdate);

    /// ホスト(canvas)がフォーカスを持っているか。
    pub fn update_host_focus_state(&mut self, is_focused: bool);

    /// ミラーを canvas の位置とスケールに合わせる。
    pub fn set_viewport(&mut self, offset: (f64, f64), pixels_per_point: f64);
}
```

`web-basics` から変える点。

- **座標を入れる。** `Node::bounding_box()`(consumer 0.38、`node.rs:315`)を読み、`position: absolute` + `left/top/width/height` を px で書く。egui のツリーは root ノードに `Affine::scale(pixels_per_point)` が乗っているので(`egui/src/context.rs:525`)、`bounding_box()` は物理 px で出てくる。**ノードごとに割り算せず、ホストに `transform: scale(1/pixels_per_point)` を一度掛けて論理 px に戻す**(Flutter が `<flt-semantics-host>` でやっているのと同じ、1.4)。
- **ホストの CSS は Flutter に倣う**(1.4)。`position: absolute` + canvas と同じ矩形、子は `position: absolute` + `overflow: visible`。透明化は **`filter: opacity(0%)` と `color: rgba(0,0,0,0)`** で、`visibility: hidden` / `display: none` は使わない(スクリーンリーダーが要素ごと無視する)。`opacity` 属性でなく `filter` なのは効かない要素があるため。既定は `pointer-events: none`(マウスは canvas に通す)で、F2 を採る要素だけ `auto` に上げる。デバッグ用に `outline: 1px solid green` を出すフラグを最初から入れる(`border` はレイアウトを動かすので使わない)。
- **DOM → `ActionRequest`。** ここが `web-basics` に丸ごと無い部分。要素ごとに listener を張り、`ActionHandler::do_action` に流す。

  | DOM イベント | 送る `ActionRequest` |
  |---|---|
  | `click` | `Action::Click` |
  | `focus`(DOM 側でフォーカスが移った) | `Action::Focus` |
  | `keydown` Enter / Space(role が button 相当) | `Action::Click` |
  | `input` / `change`(role が slider / spinbutton) | `Action::SetValue` + `ActionData::NumericValue` |
  | `change`(role が checkbox / switch) | `Action::Click`(egui は toggle を Click で受ける) |

  `target_tree` は `TreeId::ROOT`、`target_node` は要素に紐付けた `NodeId`。floers も同じ結論に達している(1.1): `ActionHandler` は adapter が実装するものではなく、**adapter が呼んで外に出すための口**である。

  **クリックの二重発火に注意。** スクリーンリーダーが合成する `click` と、ユーザーが canvas を押した本物のポインタ操作の両方が同じウィジェットに届きうる。Flutter は `ClickDebouncer` で 200ms / 50ms の時間窓を作って見分けている(1.4)。我々の場合はミラーが既定で `pointer-events: none` なので、DOM の `click` が来る = 支援技術が合成したもの、という切り分けが最初から効く。F2 で `auto` に上げた要素だけ同じ問題が出るので、そのときに考える。
- **要素の種類。** Flutter に倣い、効く場所では本物の要素を使う。`Role::Slider` → `<input type="range" role="slider">`、`Role::TextInput` → `<input>`、`Role::Link` → `<a href>`。checkbox は `role="checkbox"` + `aria-checked` の `<div>` でよい(Flutter もそうしている)。ラベルは `aria-label` 一本にしない(1.4 の `LabelRepresentation`。`aria-label` は JAWS とクローラに効かない)。まず `aria-label`、VoiceOver で足りなければテキストノードに替える。
- **Tab 順。** `is_focusable(&filter)` が真なら `tabindex="0"`(`web-basics` は `-1` 固定)。ツリー順が DOM 順なので、Tab の順序は egui のウィジェット順になる。F1 を採る場合はホスト全体を `tabindex="-1"` にして canvas 1 個だけを Tab 対象にする。
- **filter は `accesskit_consumer::common_filter` のまま。** 0.38 の `common_filter` は「親が children を clip していて、自分の矩形が親の外にあり、前後の兄弟も外なら subtree を除く」を既にやる(`filters.rs:54`)。`ScrollArea` の外に出た行はミラーにも出ない。スクロール直後の 1 つ前後だけは残すので、支援技術が `ScrollIntoView` を投げる足がかりもある。
- **role の対応表はほぼそのまま使う。** 150 行の `match` は accesskit の `Role` が増えた以外は有効。`Role::Window`(egui の root ノード)を `web-basics` は `application` にしていた(ブランチの最終コミットがまさに "Set application role on the root node")が、`role="application"` はスクリーンリーダーの browse mode を切るので、**採否を VoiceOver で確かめてから決める**。関連して、role を付けない素のテキストは Safari が勝手にマージすることが Flutter で報告されている([#166787](https://github.com/flutter/flutter/issues/166787))ので、`Role::Label` にも role を明示する。

対象ウィジェットは task.md のとおり Button / Checkbox / Label / TextEdit / Slider / ComboBox。**TextEdit だけは別扱いになる。** egui が `ActionRequest` で受けるのは `Action::SetValue`(Slider / DragValue のみ、`widgets/slider.rs:755`)と `Action::SetTextSelection`(`text_selection/cursor_range.rs:188`)で、**テキストの入力そのものを受ける経路は無い**。web では eframe の text agent(隠し `<input>`)がすでにキーボードを受けているので、TextEdit ノードのミラーは「名前と値を読み上げさせる `role="textbox"`」に留め、入力は text agent に任せる(フォーカスを text agent に渡す)。

### 2.2 ランナーが `accesskit_update` を受け取る口

3 案ある。

| 案 | やり方 | 判定 |
|---|---|---|
| A | eframe を fork し、`WebOptions` に `accesskit_sink: Option<Box<dyn FnMut(TreeUpdate)>>` を足して `handle_platform_output` から呼ぶ | fork が要る。上流に出すまで main に入らない |
| B | 自前の web ランナーを書く(`egui::Context` + `egui-wgpu` + 入力の橋渡しを全部自分で) | text agent / IME / タッチ / リサイズ / storage を全部書き直すことになる。eframe web は 15 ファイルある。**採らない** |
| C | **egui の `Plugin::output_hook` で拾う**(1.2(b)) | crates.io の eframe / egui のまま動く。native でも kittest でも同じコードが動く |

**C を採る。** 1.2 で分かったとおり、これは fork も自前ランナーも要らない。試作は `react-egui-app` の `Options::setup` から 2 行で始められる。

```rust
// spike ブランチの gallery/src/main.rs
Options {
    setup: Some(Box::new(|cc| {
        cc.egui_ctx.enable_accesskit();
        cc.egui_ctx.add_plugin(react_egui_app::a11y::WebA11y::new("react_egui_canvas"));
    })),
    ..Default::default()
}
```

`WebA11y` は `react-egui-app` 側の薄い glue(`#[cfg(target_arch = "wasm32")]`)で、`accesskit_web::Adapter` を `thread_local!` に持ち、`Plugin` の 2 つの穴を繋ぐだけ。

```rust
impl egui::plugin::Plugin for WebA11y {
    fn debug_name(&self) -> &'static str { "react_egui_web_a11y" }

    fn output_hook(&mut self, _ctx: &egui::Context, output: &mut egui::FullOutput) {
        if output.platform_output.requested_discard() { return; }   // 5.3 の捨てられるパス
        let Some(update) = output.platform_output.accesskit_update.take() else { return; };
        with_adapter(self.key, |a| a.update_if_active(|| update));
    }

    fn input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        with_pending_actions(self.key, |req| {
            input.events.push(egui::Event::AccessKitActionRequest(req));
        });
    }
}
```

**B を採らない理由をもう一つ。** ARCHITECTURE 8 章は「ライブラリ本体は `&mut egui::Ui` しか触らないので、プラットフォーム対応はランナー層に閉じる」と書いている。自前ランナーはその閉じ込めを web でだけ太らせる方向で、iOS ランナー(7 章)とは事情が違う(iOS は eframe が対応していないので選択肢が無い)。web には eframe がある。

**上流に出す形は A ではなく「eframe が adapter を持つ」。** 1.3 のとおりフォーカスの扱いは eframe の内側にあるので、コールバックだけ生やしても呼び手が正しく書けない。C は「上流が入るまでの外付け」であって、上流提案そのものではない。

### 2.3 フォーカスの往復

**ここが本タスクで唯一「誰も答えを持っていない」場所である。** AccessKit のメンテナが `web-basics` の試作を「フォーカスの移動が信頼できない」と評して止めており(1.1)、Flutter は 3 つの仕掛け(遅延 focus / blur を呼ばない / エコー抑制)でようやく回している(1.4)。試作の成果物はここの実測値だと考える。

1.3 の F1 / F2。実装順は F1 → 測る → 必要なら F2。

- egui → DOM: `TreeChangeHandler::focus_moved` で、F1 なら canvas の `aria-activedescendant` を差し替え、F2 なら `element.focus()`。**Flutter に倣って 3 点を守る**: (a) `focus()` はツリーの更新を全部反映し終えてから呼ぶ、(b) **`blur()` は呼ばない**(`web-basics` は呼んでいる)、(c) 同じノードへの再設定は握り潰す。
- DOM → egui: 要素の `focus` イベント(F2)か、canvas の `keydown` で Tab を捕まえて次のノードを自分で決める(F1)。どちらも `Action::Focus` を送る。egui は `Memory` でこれを受けて `id_requested_by_accesskit` に置く(`egui/src/memory/mod.rs:610`)。**直前に自分が要求したフォーカスの `focus` イベントは送り返さない**(エコーで無限ループになる。Flutter の `_lastEvent == requestedFocus` と同じ)。
- ホスト全体のフォーカス: `Adapter::update_host_focus_state(is_focused)` に eframe の「canvas か text agent にフォーカスがあるか」を毎フレーム渡す。plugin からは `ctx.input(|i| i.focused)` で読める。

### 2.4 いつ有効にするか

`ctx.enable_accesskit()` は毎フレーム全ウィジェットぶんの `accesskit::Node` を作るので、常時 on にはしない。Flutter web は「窓の外の 1px の `<button aria-label="Enable accessibility">` がクリックされたら」で判定している(1.4)。

**試作は常時 on にする。** 目的が「支援技術から使えるか」の確認なので、有効化の作法まで一度に確かめると原因の切り分けができない。有効化の設計は上流に持ち込むときの論点として残し、AccessKit の `ActivationHandler`(`request_initial_tree`)がすでにその形の穴なので、adapter の API はそれに合わせておく(2.1)。react-egui 側に `Options.a11y: bool` を置くのは、上流の形が決まってからでよい。

### 2.5 main に入るラベル固め

adapter の有無に関わらず効く。**「名前の無いウィジェット」はミラーがあっても読み上げようが無い**ので、先に潰す。

egui 0.36 の API を確認した結果、必要な道具は揃っている。

- `egui::Image::alt_text(impl Into<String>)` が `WidgetInfo.label` に入り、そのまま AccessKit ノードの `label` になる(`egui/src/widgets/image.rs:272, 407`)。エラー時の ⚠ プレースホルダの隣にも描かれる。
- 任意のウィジェットの名前を後から上書きするなら `Context::accesskit_node_builder(id, |node| node.set_label(..))`(`context.rs:3681`、public。accesskit が無効なら `None` を返すだけ)。`Response::widget_info` を使う手もあるが、こちらはクリックしたフレームに `OutputEvent` をもう 1 つ積んでしまうので採らない。

変更は 2 つだけにする。

| 要素 | 追加する prop | 実装 |
|---|---|---|
| `Image` | `alt: Option<&str>` | `image = image.alt_text(alt)` |
| `Button` | `label: Option<&str>` | `ui.ctx().accesskit_node_builder(resp.id, \|n\| n.set_label(label))`。children は見た目のまま |

`Checkbox` / `Slider` / `ComboBox` はすでに `label: Option<&str>` を持っている。持っているのに渡していない場所があるのが実際の問題で、`examples/todo` の行内チェックボックスがそれ(`examples/todo/src/lib.rs:118`。名前が空)。`x` ボタン(同 124 行)も「x」としか読まれない。examples を直し、**同じ穴が再発しないよう kittest で塞ぐ**(4 章)。

ARCHITECTURE / README に足す一文は次の趣旨。

- ARCHITECTURE 1 章の非ゴール: 「web のスクリーンリーダー対応は egui / AccessKit の web 対応に依存する。egui はウィジェットツリーを AccessKit に出しており native では OS のアクセシビリティ API に届くが、web ではそれを DOM に映す adapter が上流に無い(`docs/tasks/a11y/`)。」
- ARCHITECTURE 8 章(プラットフォーム): 上と同じことを 1 行、`eframe/src/web/app_runner.rs` の `accesskit_update: _` を指して書く。
- README の gallery 節: 「gallery は canvas に描いているので、web ではスクリーンリーダーから読めない。native では読める。」

## 3. 手順

### PR #4(main、済)

1. **本書と task.md の訂正。** 1.1 で分かった `web-basics` ブランチの存在を task.md の背景表に反映する(「無い」→「リリース版は無い。試作ブランチが 1 本ある」)。コミット。
2. **`Image` の `alt`、`Button` の `label`。** `crates/react-egui-elements/src/widgets.rs`。kittest を `tests/widgets.rs` に 2 本(A-1、A-2)。コミット。
3. **examples のラベル埋め。** todo のチェックボックスと `x` ボタン、他の example を一通り見て名前の無いウィジェットを潰す。snapshot が動くなら撮り直す。コミット。
4. **名前の無いノードを見つける kittest**(A-3)。gallery の全 example を 1 つずつ描き、focusable なノードに空でない名前があることを確かめる。コミット。
5. **ARCHITECTURE 1 章 / 8 章と README の一文。** README は「Examples」節の gallery の段落(`Every example but one runs in the browser ..`)の末尾に足す。コミット。PR。

### PR #5(main、この続き)

0 章のとおり、ここは fork を必要としないので main に入れる。

6. **`accesskit_web` の骨格を移植する。** `web-basics` の 4 ファイルを `crates/accesskit-web/` に置き、accesskit 0.24.1 / consumer 0.38 に合わせて直す(`name()` → `label()`、`is_focusable(&filter)`、`TreeId`)。まだ座標もイベントも無い。ビルドが通ることだけ確かめる。
7. **座標とホストの CSS**(2.1)。`bounding_box()` を `position: absolute` に落とし、canvas に重ねる。DevTools で矩形がウィジェットの上に乗っていることを目視。
8. **`react_egui_app::a11y::WebA11y`(plugin)と `Options::setup` からの起動**(2.2)。gallery を trunk でビルドし、DOM が毎フレーム更新されることを目視。捨てられたパスを弾く条件がちゃんと効いているかを、taffy が 2 パス回る example(layout)で確かめる。
9. **DOM → `ActionRequest`**(2.1 の表)。click と Enter / Space で counter の `+` が増えるところまで。
10. **フォーカス F1**(2.3)。`aria-activedescendant` 版。VoiceOver(macOS Safari / Chrome)で counter / todo / form を触る。

### `a11y-spike` ブランチ(PR #5 の後)

`main` から切る。fork した eframe はこのブランチに閉じる。

11. **F1 で足りなければ F2。** eframe を fork し、`has_focus` を「canvas の親の中に activeElement があるか」に緩める。`[patch.crates-io]` で git fork を指す。**このコミットは spike ブランチだけに置く。**

### 上流

12. **上流。** AccessKit に discussions#514 の続きとして「web-basics を現行 API に起こし、座標とイベントを足した。引き取れるか」を書く。eframe / egui に「web で AccessKit ツリーが捨てられている。adapter を eframe が持つ形を提案する」の issue を立てる。

## 4. テスト / 確認

### main(headless、CI で回る)

kittest は AccessKit ツリーをそのまま歩く(`egui_kittest::Harness::root()` が返すのは `kittest::AccessKitNode` = `accesskit_consumer::Node`)。ラベル固めは全部 headless で押さえられる。

| # | 場所 | 内容 |
|---|---|---|
| A-1 | `crates/react-egui-elements/tests/widgets.rs` | `<Image alt="a cat"/>` を `harness.get_by_label("a cat")` で引ける。`alt` 無しなら引けない |
| A-2 | 同上 | `<Button label="delete">"x"</Button>` が `get_by_role_and_label(Role::Button, "delete")` で引ける。見た目(`get_by_label("x")` が引く矩形)は変わらない |
| A-3 | `examples/gallery/tests/` | `gallery::EXAMPLES` を回して `<App start={meta.name}/>` を描き、ツリーを root から辿って「focusable かつ `label()` が空」のノードが無いことを確かめる。見つかったら example 名と role と矩形を出す。**これが 3 の再発防止** |

A-3 は「今後 example を足した人が名前を忘れたら赤くなる」ための仕掛けなので、失敗メッセージに「`label` を渡してください」と書く。窓は既存の `examples/gallery/tests/gallery.rs` と同じ 1280×1000 にする(`ScrollArea` が描かなかったウィジェットはツリーにも居ないので、小さい窓だと見逃す)。

### web(手で確かめる)

wasm の自動テストは `wasm-bindgen-test` で DOM の形(ノード数、`role`、`aria-label`、矩形)までは書けるが、**支援技術が実際にどう読むかは自動化できない**。VoiceOver の挙動が全てなので、チェックリストを置いて手で回す。

- `wasm-bindgen-test`(headless Chrome): `TreeUpdate` を手で作って `Adapter` に流し、DOM に期待どおりの要素と属性と座標が並ぶこと。差分更新でノードが増減すること。adapter は egui 非依存なので、このテストに egui は出てこない。
- VoiceOver チェックリスト(macOS、Safari と Chrome の両方。gallery の counter / todo / form)。

  **状態: 回した(2026-09-05、macOS)。8 項目とも通った。F1 で成立し、手順 11(F2、eframe fork)は不要。** 1 は最初 `Cmd+Option+→` を押していてブラウザのタブ切替になっただけで、VO キー(`Ctrl+Option`)で動いた。canvas が `role="application"` なので、VO カーソルが中に降りない場合は `VO+Shift+↓` で一段入る。ブラウザの版は未記録。

  | | 確かめること | 結果 |
  |---|---|---|
  | 1 | VO+右矢印でウィジェットを順に読み、ボタン名が読まれる | ○ |
  | 2 | Tab でフォーカスが移り、VoiceOver のカーソルが追随する | ○ |
  | 3 | Enter / Space でボタンが押され、結果(カウンタの値)が読まれる | ○ |
  | 4 | チェックボックスの「オン/オフ」が読まれ、切り替えられる | ○ |
  | 5 | テキスト欄に入力でき、入力した値が読まれる | ○ |
  | 6 | スライダーの値が読まれ、矢印キーで動かせる | ○ |
  | 7 | ミラーがマウス操作を邪魔していない(`pointer-events`。Flutter が [#188859](https://github.com/flutter/flutter/issues/188859) / [#160560](https://github.com/flutter/flutter/issues/160560) で踏んでいる穴) | ○ |
  | 8 | フォーカスされているノードが**目でも**分かる(egui 側のフォーカスリングが出ている)。Flutter web は透明なミラーのせいでこれが出ず、[#186044](https://github.com/flutter/flutter/issues/186044) が open のまま | ○ |

  読み上げの様子は動画に録って上流の issue に貼る。

## 5. 終了条件との対応

task.md の終了条件は 3 つある。3 番目は PR #4 で済み、1 番目は手順 10(必要なら 11)、2 番目は手順 12。

| 終了条件 | どこで |
|---|---|
| gallery(web)の counter / todo / form を VoiceOver で読み上げ・Tab・Enter / Space | 手順 10(必要なら 11)、4 章のチェックリスト |
| 上流に web adapter と eframe の差し込み口の提案が出ている | 手順 12 |
| ARCHITECTURE.md に web の a11y の現状と方針が書かれている | 手順 5(**PR #4 で済**) |

## 6. 実装で判明した差分

手順 2〜5(PR #4、ラベル固め)は 6.1〜6.5、手順 6〜9(PR #5、ミラー本体)は 6.6〜6.9、手順 10(フォーカス F1)は 6.10。

### 6.1 `Button` の `label` は 3 章のとおり。`Image` の `alt` も同じ

`ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(label))` は egui 0.36.1 でそのまま通る(`Context::accesskit_node_builder(Id, impl FnOnce(&mut accesskit::Node) -> R) -> Option<R>`)。ウィジェットが自分のノードを書いた後に呼べば名前を上書きでき、accesskit が無効なら `None` が返るだけで何も起きない。描くものは変わらないので、生 egui 版と並べる example でもピクセルが動かない。

### 6.2 名前を付けられないウィジェットが残った

A-3(`examples/gallery/tests/a11y.rs`)を書いて全 example を走らせたところ、focusable で名前が空のノードが 13 個出た。**そのどれも、今の要素の API では「描くものを変えずに」名前を付けられない。**

| 出たもの | なぜ付けられないか |
|---|---|
| `TextInput`(showcase / todo / form / custom-hook / list-10k) | `egui::TextEdit` は AccessKit ノードに label を一切書かない。`hint_text` は `PlatformOutput`(web screen reader 用の読み上げ文)に行くだけでノードには乗らない。`<TextEdit>` にも名前の prop が無い |
| `form` の `CheckBox` ×2 / `Slider` / `SpinButton` / `ComboBox` | ラベルは隣の列(`<Field>`)が描いている。AccessKit ではこれは `labelled_by` 関係で、要素は `Response::labelled_by` を出していない。各要素が持つ `label` prop は**文字を描く**ので、名前が画面に 2 回出て、隣に並ぶ生 egui 版ともズレる |
| `escape-hatch` の `ColorWell` | 生の `ui.color_edit_button_srgba`。egui が名前を付けていない |
| `shader` の `Unknown` | `<Canvas>` の leaf。コントロールではなく描画面なので、canvas 一般の扱いを決めてからにする |

そこで A-3 は「focusable で名前が空のノードが 0」ではなく、**この 13 個を `KNOWN_UNNAMED` として並べ、完全一致で比べる**形にした。名前の無いウィジェットが増えれば落ち、既知のものを直したときも(表から消し忘れれば)落ちる。リストは減る方向にしか動かない。

続きとして要るもの。どちらも要素の API を増やす話なので、この PR には入れない。

1. `<TextEdit>` に `Button` と同じ「描かない名前」を足す。上の 5 個が消える。
2. `<Field>` のような「隣のラベル」を AccessKit に繋ぐ道(`Response::labelled_by` 相当)。form の 5 個が消える。

### 6.3 `examples/todo` の行内チェックボックスは直せなかった

2.5 で「実際の問題」と書いた場所。名前を与える唯一の口である `label` prop は文字を描くので、行の見た目が変わり、同じ絵と比べている生 egui 版(と `snapshot` の画像)とズレる。6.2 の 1 と同じ「描かない名前」が要る。なお既定の todo は空リストで始まるので、この行は A-3 のツリーには出てこない。

`x` ボタン(2.5 のもう 1 つ)は `label="remove"` で直した。生 egui 版にも同じ名前を `accesskit_node_builder` で手で付けてある。両方を同じ手順で driving しているテストがラベルで引いているため、かつ「同じアプリ」であるべきだからである。`list-10k` の `x` も同じ。

### 6.4 `+` / `-` はそのままにした

counter と custom-hook のボタン。名前は空ではない(「プラス」「マイナス」と読まれる)し、隣に数が出ているので意味は通る。直すと生 egui 版と react-egui 版を同じ手順で driving しているテストが片方だけズレるため、費用の方が大きいと判断した。

### 6.5 A-3 は `run` ではなく `run_steps(2)`

`shader`(と `clock`)は毎フレーム再描画を要求するので、`Harness::run` が「4 ステップで落ち着かない」と panic する。`fetch` は描いた瞬間に本物の HTTP を投げるので、既存の `gallery.rs` と同じ理由で外してある。

### 6.6 手順 6: 移植で変わったところ

- `accesskit_consumer` 0.38 でも `TreeChangeHandler` / `TreeState` は `ChangeHandler` / `State` の別名として残っていたので、そのまま使えた。変わるのは `HashMap` のキーで、`accesskit::NodeId` ではなく**木の index を含む consumer 側の `NodeId`** になる。
- `Role::Directory` は accesskit 0.24 に無いので role 表から落とした。他の 150 行はそのまま通る。
- `Adapter::new` は `-> Self` ではなく **`-> Option<Self>`**。`window()` / `document()` の `unwrap()` を wasm に持ち込まないため。親も id 文字列ではなく `&Element` で受ける(2.1 のとおり)。`set_attribute` の `unwrap()` も全部落とした(属性名は定数なので失敗しない)。
- **`focus_moved` は空にした。** `element.focus()` は 1.3 の壁そのもので、`blur()` は 1.4 の教訓から呼ばない。egui → DOM のフォーカスは手順 10 の仕事。
- root のホストに `role="application"` は付けていない(2.1 の保留どおり)。egui の root ノードが `role="window"` として 1 段下に出る。
- **native でも `web-sys` はビルドが通る**ので、crate 全体を `cfg` で切らずに置いた。workspace の `clippy --all-targets` と `test` が accesskit-web も見る。
- clippy の `large_enum_variant` に言われて `State::Active` の `Tree` は `Box` に入れた。

### 6.7 手順 7: 座標

- **子の `left/top` は親の矩形からの相対にする必要があった。** 絶対配置の親が絶対配置の子の包含ブロックになるので、`bounding_box()` をそのまま書くと入れ子のぶんだけ二重にずれる。矩形を持たない祖先は飛ばし、一番近い「矩形を持つ祖先」を原点にする。Flutter の `recomputeChildrenAdjustment` と同じ話で、2.1 に書き落としていた。
- デバッグ表示は `Adapter::set_debug(bool)`。`filter: opacity(0%)` を外して緑の `outline` を出す。plugin 側は URL に `?a11y-debug` があれば on にする。
- `tabindex` は `is_focusable(&filter)` が真なら `"0"`。ただしこれで Tab がミラーに入るようになるので、1.3 の壁に当たるのは手順 10 の宿題として残る。
- `Role::Label` に `role="paragraph"` を明示した(1.4 の Safari のマージ対策)。
- **`Role::Label` の文字は `label()` ではなく `value()` に入っている**(`Node::label_comes_from_value`。egui もそう書いている)。ブラウザで見るまで気付かず、ミラーの文字が全部空だった。同じ文字を `aria-valuetext` に重ねて出さないようにもした。

### 6.8 手順 8: plugin

- **捨てられるパスの判定は `requested_discard()` だけでは足りない。** `max_passes` を使い切った最後のパスでもフラグは立ったままで、そこで捨てると最終形が DOM に出ない。`Context::run` のループの抜け条件と同じ `requested_discard() && num_completed_passes < max_passes` にした。**`ctx.will_discard()` は使えない**: `end_pass` が viewport の output を `mem::take` した後に plugin が呼ばれるので、常に false になる。
- `Options::setup` は `FnOnce` が 1 本なので、gallery では `shader::gpu::setup(cc)` と `add_plugin` を同じクロージャに並べた。
- `accesskit` は `egui::accesskit`(egui の再エクスポート)を使う。版がずれようがない。`accesskit-web` は wasm32 の依存にだけ入れ、native の `WebA11y` は空の plugin にした。

### 6.9 手順 9: DOM → `ActionRequest`

- 要素ごとに listener を張らず、**ホスト 1 枚に張って委譲**した。要素には `data-accesskit-node` / `data-accesskit-tree` を書いておき、イベントはそこから親を辿って target を作る。ノード数ぶんのクロージャを持たずに済む。`focus` は bubble しないので `focusin` を使う。
- `ActionHandler` は `Rc<RefCell<Box<dyn ActionHandler>>>` で listener と共有する。plugin 側の実装はキューに積むだけで、`input_hook` が `RawInput` に流す。
- **`ctx.request_repaint()` が要る。** egui は必要なときしか描かないので、キューに積んだだけでは次のフレームが来ない。支援技術のクリックは egui にとって「何も起きていない」のと同じ。これが無いと押しても無反応で、実測するまで気付かなかった。
- 2.1 の表のうち `input` / `change` は、ミラーが今 `<div>` なので実際には飛んでこない(本物の `<input>` にしたときのため)。チェックボックスの toggle は合成 `click` の方で届く。
- **headless Chrome での実測**(gallery、`?a11y-debug`): ミラーの button を `click()` すると example が切り替わり、counter の `+` で数が増える。`keydown` の Enter / Space も効く。`elementFromPoint` はミラーではなく canvas を返す(`pointer-events: none` が効いている)。
- `crates/accesskit-web/tests/mirror.rs` は `#![cfg(target_arch = "wasm32")]` で囲ってあるので `cargo test --workspace` は素通りする。`wasm-pack test --headless --chrome crates/accesskit-web` で 4 本通る。

### 6.10 手順 10: フォーカス F1 と、値ウィジェットの本物の要素

**DOM → egui は書くものが無かった。** 2.3 は「canvas の `keydown` で Tab を捕まえて次のノードを自分で決める」と書いていたが、実測すると **eframe と egui がすでに両方やっていた**。

- eframe の `keydown` listener は canvas に張ってあり(`web/events.rs:83`)、Tab を `egui::Key::Tab` としてそのまま egui に渡したうえで `prevent_default()` する(同 269 行、コメントに「egui uses Tab to move focus within the egui app」とある)。ブラウザが次の HTML 要素にフォーカスを移すことは無い。
- egui 側は `Memory::begin_pass` が Tab / Shift+Tab を `FocusDirection::Next` / `Previous` に変え(`memory/mod.rs:596`)、`end_pass` でウィジェットを選ぶ。

つまり **F1 の DOM → egui 方向は「何もしない」が正解**で、自前の Tab 走査を書くと egui と二重に動く。手順 10b はミラー側では 1 行も書かず、`focus_moved` で `aria-activedescendant` を追随させるだけにした。エコー抑制も要らない(ミラーは `focus()` を呼ばないので、返ってくるフォーカスイベントが無い)。

**`aria-activedescendant` を正当にするのに 2 つ足りなかった。** 調べた結果、参照先は「参照元の DOM の子孫」か「`aria-owns` で参照元が所有している要素」のどちらかである必要がある(WAI-ARIA 1.2 の `aria-activedescendant`、APG の "Developing a Keyboard Interface")。ミラーのホストは canvas の**兄弟**なので、canvas に `aria-owns="<host id>"` を付けた。さらにこの属性が許されるのは `application` / `combobox` / `composite`(とその派生)/ `group` / `textbox` の role だけで、素の `<canvas>` はどれでもない。そこで **canvas に `role="application"` を付けた** — 2.1 で「VoiceOver で確かめてから決める」と保留にしていた判断を、ここで採ったことになる。Chrome の a11y ツリーでミラー全体が canvas(application)の下にぶら下がることは確認した。

- **代案として「ミラーを canvas の子にする」**(canvas fallback content)がある。子孫になるので `aria-owns` も `role` も要らない。採らなかったのは、1.5 のとおり fallback content の要素は**描画ボックスを持たない**ので、我々が手順 7 で入れた座標が意味を失うからである。上流に出すときの選択肢としては残る。
- **支援技術の対応は薄い。** VoiceOver + Safari は **Safari 18(2024)で直るまで `aria-activedescendant` を無視していた**([WebKit#167680](https://bugs.webkit.org/show_bug.cgi?id=167680))。`aria-owns` は macOS 14.3 / iOS 17.3 より前の VoiceOver に出ない。iOS / Android のタッチ系支援技術は a11y ツリーを直接なぞるので、この属性を事実上見ない。**F1 は「今の Safari なら通るかもしれない」程度**で、1.3 の見立て(F2 が本命)は変わっていない。4 章の表を人が埋めるまで結論は出ない。

**1.4 の 3 つの規則はそのまま守った。**(a) `focus_moved` は**記録するだけ**にし、属性を書くのは `update_and_process_changes` が返った後。consumer は `focus_moved` を `node_removed` より前に呼ぶので、記録しないと消える途中の DOM を指しうる。(b) `blur()` に当たるのは「アプリがフォーカス無しを報告したときに属性を消すこと」なので、**消さない**。(c) 同じノードへの再設定は握り潰す。例外は 1 つだけで、**参照先の要素が木から消えたときは属性を外す**(宙に浮いた参照は無いより悪い)。

**`aria-selected` は付けなかった。** 10 の指示にはあったが、この属性は listbox の option や grid の row など一部の role でしか意味を持たず、button に付ければ支援技術に嘘をつくことになる。フォーカスの主張は canvas の `aria-activedescendant` 1 本に絞り、ミラー側は **`data-focused="true"`** という印だけにした。`?a11y-debug` では、フォーカス中のノードだけ枠をマゼンタにする(他は緑のまま)。

**`tabindex` は `"0"` から `"-1"` に落とした**(ホストも `-1`)。手順 7 の宿題(6.7 の最終行)がこれで片付く。focusable なノードに属性を出すこと自体は残してあるので、「どれがアプリのフォーカス先か」は DOM を見れば分かる。

**値ウィジェットは本物の要素にした**(手順 9 で残した穴、6.9 の 4 点目)。

- `Role::Slider` / `Role::SpinButton` → `<input type="range">`。`min` / `max` / `step` はノードの数値から。`step` が無いと range は整数に丸めるので、無いときは `any` を入れる。
- `Role::TextInput` → `<input type="text" readonly>`。**readonly はミラーが打鍵を取らないため**で、入力は 2.1 のとおり eframe の text agent に任せる。
- **値は属性ではなくプロパティで書く。** 支援技術が range を動かした後は、`value` 属性は `defaultValue` にしかならず、表示が egui から離れる。DOM の値がアプリの値と違うときだけ上書きする形にした。
- `role` 属性はそのまま残してあるので、6.9 で書いた `NUMERIC_ROLES` の判定(`input` / `change` → `SetValue`)がそのまま効く。`aria-valuenow` / `valuemin` / `valuemax` も全ノードに出したままで、`<div>` のままの role の取り分になる。
- checkbox は Flutter と同じく `<div role="checkbox">` + `aria-checked` のまま。
- role が変わって `<div>` と `<input>` を跨ぐときは要素を作り直し、子要素は移し替える(タグは後から変えられないため)。egui のノード id はウィジェットごとに安定なので実際にはまず起きない。

**headless Chrome での実測**(`trunk build` した gallery を配って `?a11y-debug`)。

- **counter**: canvas をクリックしてから Tab を送ると、canvas の `aria-activedescendant` が `window` → `showcase` → `counter` と動き、Shift+Tab で戻る。**`document.activeElement` は canvas のまま**で、`data-focused` の付いたノードと参照先は常に一致する。`+` まで Tab して Enter を押すとカウントが 1 → 2 に増えた。
- **form**: ミラーの slider は `<input type="range" min="0" max="100" step="1">` で値は 50。`value = 80` にして `input` イベントを投げると、次のフレームで egui 側が 80 になった(要約行が「volume 80」)。TextEdit は `<input type="text" readonly value="anon">`。
- **`elementFromPoint` は canvas を返す。** `pointer-events` は継承されるので、ホストの `none` が `<input>` にも効いている(4 章の 7 番目、Flutter が踏んだ穴)。
- **canvas を blur しても `aria-activedescendant` は残り、戻しても同じ**。`data-focused` だけが消えて戻る(10d の確認)。
- **TextEdit にフォーカスが移った瞬間だけ `document.activeElement` が `<input>` になる**が、これはミラーではなく **eframe の text agent** である。eframe の `has_focus` はこれも「フォーカスあり」と数えるので(1.3)、問題は起きない。
- コンソールにエラーは出ない。

**残っている穴。**

- **VoiceOver の実機は 8 項目とも通った**(4 章)。F1 のまま上流に出す。
- `Role::MultilineTextInput` は `<div role="textbox">` のまま。`<textarea>` にするかは、テキスト欄の読み上げを一度見てから決める。
- form の slider の隣に出る `SpinButton`(egui の `DragValue`)は `step` がドラッグの刻み(1.12…)になる。矢印キーで動かす分には粗すぎるので、`numeric_value_step` を使うかどうかは role ごとに分ける余地がある。
- `aria-owns` はミラーを canvas の下に付け替えるので、canvas 自身の子(将来 eframe が何か置いたら)との順序は保証しない。
