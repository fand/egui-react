---
title: 要素
---

# 要素

このページのものはすべて `egui_reactor_elements::prelude` から来ます。生成されたイベント enum も、そこから export されています。`<Button on_click=../>` を書く場所には `ButtonEvent` がスコープに要ります。

どの要素も `style` prop を取ります（レイアウト上のボックスを持たない`<Window>` と `<Row>` は例外です）。`rsx!` は[レイアウト属性](/ja/reference/layout-attributes) に並べたレイアウトと描画の属性からそれを埋めます。下の表では繰り返しません。

## レイアウト

### `<View>`

flex または grid のコンテナです。それ自体が 1 つのレイアウトノードで、子もすべてノードになります。

| Prop | 型 | 意味 |
|---|---|---|
| `display` | `Display` | `"flex"`（既定）、`"grid"`、`"block"`、`"none"` |
| `direction` | `Direction` | `"row"`（既定）、`"column"`、`"row-reverse"`、`"column-reverse"` |
| `wrap` | `bool` | 子を複数行に折り返す |
| `justify` | `Justify` | 主軸: `"start"`、`"center"`、`"end"`、`"space-between"`、`"space-evenly"`、`"space-around"`、`"stretch"` |
| `align` | `Align` | 交差軸: `"start"`、`"center"`、`"end"`、`"baseline"`、`"stretch"` |
| `align_content` | `Option<Justify>` | 折り返した行の交差軸での分配 |
| `gap` | `Gap` | 数値 1 つ、または `(column, row)` |
| `cols` | `Option<u16>` | 等幅の列。`display="grid"` のときだけ |
| `children` | `impl View` | |

```rust
<View direction="row" justify="space-between" align="center" gap={8} p={12}>
    <Text grow={1.0}>"Title"</Text>
    <Button on_click={|| *open = true}>"Open"</Button>
</View>
```

使っているサンプル: 全部。

### `<Text>`

レイアウトエンジンが自分で測って描くテキストノードです。専用の `Ui` も`Label` も持たないので、木にテキストを置くいちばん安い方法です。

| Prop | 型 | 意味 |
|---|---|---|
| `size` | `Option<f32>` | フォントサイズ（ポイント） |
| `color` | `Option<egui::Color32>` | |
| `strong` | `bool` | |
| `wrap` | `bool` | 既定はオフ。だからテキストは縮まずに全幅を申告する。折り返すには折り返す先の幅が要る |
| `selectable` | `Option<bool>` | スタイルの `interaction.selectable_labels` を上書きする |
| `font` | `Option<&str>` | 登録したフォントスタックの名前、または `"proportional"` / `"monospace"` |
| `children` | `impl Into<egui::WidgetText>` | |

```rust
<Text size={32.0} strong>{format!("{}", *count)}</Text>
<Text wrap w="100%" font="ui">{paragraph}</Text>
```

使っているサンプル: [counter](/ja/examples/counter)、[board](/ja/examples/board)、[font](/ja/examples/font)、ほかほとんど全部。

## ウィジェット

以下はどれも、egui のウィジェットを 1 つ、レイアウトの葉として描きます。

### `<Button>`

| Prop | 型 | 意味 |
|---|---|---|
| `enabled` | `bool` | 既定は `true` |
| `label` | `Option<&str>` | 子がアイコンや記号のときの、支援技術に読ませる名前 |
| `children` | `impl Into<egui::WidgetText>` | 描かれるもの |
| `on_click` | イベント `()` | |

```rust
<Button label="close menu" on_click={|| *open = false}>"×"</Button>
```

`label` は描かれるものを変えません。アクセシビリティツリーの名前を差し替えるだけです。子が `"×"` のボタンは、指定しないと「かける」などと読み上げられてしまうので、渡してください。`<Button>` に付けた `p` は、上下左右が同じでポイント指定なら、そのウィジェット自身のパディングになります。だから押しを受けるボックスと、描いたボックスが一致します。

使っているサンプル: [counter](/ja/examples/counter)、[todo](/ja/examples/todo)、[form](/ja/examples/form)、ほかほとんど全部。

### `<Label>`

葉に入った `egui::Label` です。`<View>` の中ではたいてい `<Text>` のほうが良い選択です。`<Label>` は egui 自身のコンテナの中で使うものです。

| Prop | 型 | 意味 |
|---|---|---|
| `wrap` | `bool` | `<Text>` と同じく既定はオフ |
| `children` | `impl Into<egui::WidgetText>` | |

使っているサンプル: [layout](/ja/examples/layout)。

### `<TextEdit>`

| Prop | 型 | 意味 |
|---|---|---|
| `bind` | `&mut String` | テキスト。`state.bind()` を渡す |
| `multiline` | `bool` | |
| `hint` | `Option<&str>` | プレースホルダ |
| `desired_width` | `Option<f32>` | レイアウトが与える幅を上書きする |
| `rows` | `Option<usize>` | 複数行のときの行数 |
| `clear_on_submit` | `bool` | `on_submit` のあとに中身を空にする |
| `on_change` | イベント `()` | このフレームでテキストが変わった |
| `on_submit` | イベント `String` | Enter が押された。ペイロードはテキスト |

```rust
<TextEdit grow={1.0} bind={draft.bind()} hint="what needs doing" clear_on_submit
    on_submit={|text: String| dispatch.send(Msg::Add(text))}/>
```

`<View>` の中では自分のノードを埋めます。1 行ならノードの幅、複数行なら縦横どちらも埋めます。`desired_width` や `rows` を指定した場合はそちらが優先です。

使っているサンプル: [todo](/ja/examples/todo)、[form](/ja/examples/form)、[fetch](/ja/examples/fetch)、[board](/ja/examples/board)、[spreadsheet](/ja/examples/spreadsheet)。

### `<Checkbox>`

| Prop | 型 | 意味 |
|---|---|---|
| `bind` | `&mut bool` | |
| `label` | `Option<&str>` | |
| `on_change` | イベント `bool` | 新しい値 |

```rust
<Checkbox bind={done.bind()} label="done"/>
```

使っているサンプル: [form](/ja/examples/form)、[todo](/ja/examples/todo)、[clock](/ja/examples/clock)、[shader](/ja/examples/shader)。

### `<Slider>`

`T: egui::emath::Numeric`（整数と浮動小数の各型）でジェネリックです。`T` は束縛したものから推論されます。

| Prop | 型 | 意味 |
|---|---|---|
| `bind` | `&mut T` | |
| `range` | `RangeInclusive<T>` | |
| `label` | `Option<&str>` | |
| `on_change` | イベント `()` | |

```rust
<Slider bind={amount.bind()} range={0.0..=1.0} label="amount"/>
```

使っているサンプル: [form](/ja/examples/form)、[shader](/ja/examples/shader)、[list-10k](/ja/examples/list-10k)、[escape-hatch](/ja/examples/escape-hatch)。

### `<ComboBox>`

| Prop | 型 | 意味 |
|---|---|---|
| `bind` | `&mut usize` | 選ばれている選択肢のインデックス |
| `options` | `&[S]`（`S: AsRef<str>`） | |
| `label` | `Option<&str>` | |
| `on_change` | イベント `usize` | 新しいインデックス |

使っているサンプル: [form](/ja/examples/form)、[patch](/ja/examples/patch)。

### `<Image>`

| Prop | 型 | 意味 |
|---|---|---|
| `source` | `egui::ImageSource<'_>` | `egui::include_image!(..)`、URI、バイト列 |
| `fit` | `Option<egui::Vec2>` | 収める大きさ |
| `alt` | `Option<&str>` | 支援技術に読ませる名前。読み込みに失敗したときは警告マークの横にも描かれる |

`alt` を埋めておく理由は、`<Button>` の `label` と同じです。

### `<Separator>`

| Prop | 型 | 意味 |
|---|---|---|
| `vertical` | `bool` | 既定は水平 |

使っているサンプル: [todo](/ja/examples/todo)、[font](/ja/examples/font)、[theme](/ja/examples/theme)、ほか。

## コンテナ

### `<ScrollArea>`

| Prop | 型 | 意味 |
|---|---|---|
| `horizontal` | `bool` | 横にもスクロールする |
| `vertical` | `bool` | 既定は `true` |
| `max_h` | `Option<f32>` | 要求する高さの上限 |
| `children` | `impl View` | |

```rust
<ScrollArea grow={1.0}>
    {rows}
</ScrollArea>
```

大きさを申告せず、与えられた空間を埋めます。だから高さの決まった親の下で`grow` を付けるか、自分で高さを持つ必要があります。[レイアウト](/ja/guide/layout) のつまずきどころを見てください。長いリストには`<VirtualList>` を使います。

使っているサンプル: [board](/ja/examples/board)、[notes](/ja/examples/notes)、[fetch](/ja/examples/fetch)、[font](/ja/examples/font)。

### `<VirtualList>`

見えている行だけを描き、残りの高さは確保だけします。だからフレーム時間が行数に依存しません。

| Prop | 型 | 意味 |
|---|---|---|
| `rows` | `usize` | 行数 |
| `row_h` | `f32` | どの行もちょうどこの高さでレイアウトされる |
| `row_w` | `Option<f32>` | 各行をレイアウトする幅、および横スクロールの範囲。既定はビューポートの幅 |
| `horizontal` | `bool` | 横にスクロールする |
| `render` | `impl for<'a, 's, 'u> FnMut(&'a mut Cx<'s, 'u>, usize)` | `i` 行目を描く |
| `on_scroll` | イベント `egui::Vec2` | 行を描いたあとの、このリストのスクロール位置 |

```rust
<VirtualList
    grow={1.0}
    rows={filtered.len()}
    row_h={24.0}
    render={|cx: &mut Cx<'_, '_>, row: usize| {
        rsx! { <Text>{filtered[row].as_str()}</Text> }.show(cx);
    }}
/>
```

行はそれぞれ専用のスコープの中で描かれるので、`for` に `key={i}` を付けたときと同じように、行がフックを持てます。どの行も同じ高さでなければいけません。`render` の境界は、上のように省略せず書いてください。prop の中で省略されたライフタイムは props 構造体自身のものに書き換えられ、ここではコンパイルが通りません。

`on_scroll` は毎フレーム発火します。だから位置を保存するハンドラは、値が変わったときだけ書いてください。毎フレーム書けば、毎フレーム再描画を要求することになります。

使っているサンプル: [list-10k](/ja/examples/list-10k)、[spreadsheet](/ja/examples/spreadsheet)。

### `<Collapsing>`

| Prop | 型 | 意味 |
|---|---|---|
| `header` | `&str` | |
| `default_open` | `bool` | |
| `children` | `impl View` | |

使っているサンプル: [todo](/ja/examples/todo)、[form](/ja/examples/form)。

### `<Frame>`

| Prop | 型 | 意味 |
|---|---|---|
| `children` | `impl View` | |

専用の props はありません。木の中では、描画と `Ui` 1 つを持つ `<View>` と同じです。素の `Ui` の上では、`style` から `egui::Frame` を組み立てます。塗り、ストローク、角の丸み、影、それに内側のマージンとしての `p`。そこで `<View>` にできない唯一のことが、これです。

### `<Window>`

浮いている egui のウィンドウです。`style` はありません。自分のレイヤーを持ち、レイアウトの中で場所を取らないからです。

| Prop | 型 | 意味 |
|---|---|---|
| `title` | `&str` | |
| `open` | `Option<&mut bool>` | 閉じるボタンが付き、押されると `false` を書き込む |
| `resizable` | `bool` | 既定は `true` |
| `default_pos` | `Option<egui::Pos2>` | |
| `default_size` | `Option<egui::Vec2>` | |
| `children` | `impl View` | |

`<Window>` を描かない、あるいは `open={false}` にすると、その子はアンマウントされます。

### `<Overlay>`

要素になった `egui::Area` です。自分のレイヤーを持ち、下にあるすべての上に描かれ、周りのレイアウトでは場所を取りません。

| Prop | 型 | 意味 |
|---|---|---|
| `anchor` | `Option<Anchor>` | `"top-left"`、`"top"`、`"top-right"`、`"left"`、`"center"`、`"right"`、`"bottom-left"`、`"bottom"`、`"bottom-right"`（egui 式の `"right-bottom"` の順も受け付ける） |
| `offset` | `(f32, f32)` | そのアンカーからずらす |
| `pos` | `Option<egui::Pos2>` | 手で位置を決める。`anchor` より優先 |
| `order` | `Order` | `"background"`、`"middle"`、`"foreground"`（既定）、`"tooltip"` |
| `constrain` | `bool` | ウィンドウの内側に収める。既定は `true` |
| `top` | `bool` | 毎フレーム、ほかのオーバーレイより上に持ち上げる |
| `fill` | `Option<egui::Color32>` | 敷きの色 |
| `children` | `impl View` | |

`w` か `h`、またはその両方を与えると **サイズ付き** になります。敷きを描き、その上に落ちた押しをすべて受け取り、自分のレイアウト木の根になります。だから中の `<View w="100%" h="100%">` はそれを埋めます。どちらも与えなければ**サイズ無し** です。子の分だけの大きさで、`fill` を渡さないかぎり何も描かず、子の横に落ちた押しはすべて下に通します。

```rust
<Overlay anchor="bottom-right" offset={(-16.0, -16.0)}>
    <Button label="new note" px={16.0} py={12.0} radius={24.0} shadow
        on_click={|| *composing = true}>"+"</Button>
</Overlay>
```

使っているサンプル: [patch](/ja/examples/patch)。canvas の上に重ねた 2 つの数値の表示に使っています。

### `<Panel>` と `<CentralPanel>`

ドッキングするパネルです。パネルは、いちばん近い egui の `Ui` ― いまの木を始めた `Ui`、ランナーの下ではウィンドウそのもの ― から場所を切り取ります。だから `<View>` の奥深くに書いた `<Panel>` も、ウィンドウの縁まで飛んでいきます。ドッキングとはそういう意味です。

| Prop | 型 | 意味 |
|---|---|---|
| `side` | `Side` | `"left"`（既定）、`"right"`、`"top"`、`"bottom"` |
| `default_size` | `Option<f32>` | 幅または高さ。どちらかは `side` による |
| `resizable` | `bool` | 既定は `true` |
| `children` | `impl View` | |

`<CentralPanel>` が取るのは `style` と `children` だけで、ドッキングしたパネルが残した場所を埋めます。このサイトのサンプルはどれもこれらを使っていません。サンプルはページの中の箱で動きますが、パネルが欲しいのはウィンドウだからです。

### `<Vertical>`、`<Horizontal>`、`<Grid>`、`<Row>`

egui 自身のコンテナを、そのほうが安いか馴染みがある場面のために葉として残したものです。その子はレイアウト木ではなく、素の `Ui` モードで描かれます。

| 要素 | Props |
|---|---|
| `<Vertical>` | `children` |
| `<Horizontal>` | `children` |
| `<Grid>` | `cols: Option<usize>`、`striped: bool`、`children` |
| `<Row>` | `children` ― `<Grid>` の 1 行 |

```rust
<Grid cols={2}>
    <Row>
        <Label>"grid a1"</Label>
        <Label>"grid b1"</Label>
    </Row>
</Grid>
```

使っているサンプル: [layout](/ja/examples/layout)。

## 描画

### `<Canvas>`

自分では何も描かない葉です。レイアウトからもらった矩形を確保して、こちらに渡します。

| Prop | 型 | 意味 |
|---|---|---|
| `sense` | `egui::Sense` | 既定は `Sense::hover()`。`on_drag` には `Sense::drag()` が要る |
| `paint` | `impl for<'a> FnOnce(&'a mut egui::Ui, egui::Rect)` | その矩形に描く |
| `on_drag` | イベント `egui::Vec2` | ドラッグの差分 |
| `on_hover` | イベント `egui::Pos2` | 矩形の中でのポインタ位置 |

```rust
<Canvas w="100%" h={240.0} paint={move |ui: &mut egui::Ui, rect: egui::Rect| {
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(rect, callback));
}}/>
```

prop の名前は `on_paint` ではなく `paint` です。`rsx!` は `on_` で始まる属性をすべてイベントとして扱うからです。境界を省略せず書く理由は、`<VirtualList>` の`render` と同じです。

使っているサンプル: [shader](/ja/examples/shader)。[patch](/ja/examples/patch) も同じやり方で絵を描きますが、手書きの葉から描いています。その葉がパッチのキャンバスも兼ねているからです。

## 非同期

### `<Suspense>`

| Prop | 型 | 意味 |
|---|---|---|
| `fallback` | `impl View` | 下に保留中のものがある間、代わりに描かれる |
| `children` | `impl View` | |

```rust
<Suspense fallback={view(|cx| { cx.ui().spinner(); })}>
    <Response url={url.as_str()} attempt={*attempt}/>
</Suspense>
```

下にある `use_future` が 1 つでも保留の間は、子の代わりに `fallback` を描きます。[非同期](/ja/guide/async) を見てください。

使っているサンプル: [fetch](/ja/examples/fetch)、[patch](/ja/examples/patch)。

## アクセシビリティ

`<Button>` の `label` と `<Image>` の `alt` が、支援技術の読む名前です。`label` は描かれるものを変えません。それは子の仕事です。だからアイコンのボタンには必ず要ります。いまのところネイティブで動き、Web では[Web とネイティブ](/ja/guide/web-and-native) で説明した DOM の写しを通して動きます。

## もっと知る

要素の一覧と、それぞれのラッパーを作った理由は[docs/ARCHITECTURE.md](https://github.com/fand/egui-reactor/blob/main/docs/ARCHITECTURE.md#elements-list-egui-reactor-elements)の 6 章にあります（英語）。
