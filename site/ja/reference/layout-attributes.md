---
title: レイアウト属性
---

# レイアウト属性

どの要素も `ItemStyle` 型の `style` prop を取り、`rsx!` が下の属性からそれを埋めます。`<View>` はさらにコンテナ属性を取ります。こちらは **子** をどう配置するかを決めます。すべて `egui_reactor::layout` にあり、prelude から再 export されています。

```rust
<View direction="column" gap={8} p={12} bg={egui::Color32::from_gray(30)} radius={8.0}>
    <Text grow={1.0} w="100%">"item"</Text>
</View>
```

## 長さ

サイズ、マージン、パディングはすべて `Length` です。

| 書き方 | 意味 |
|---|---|
| `{12}` / `{12.0}` | egui のポイントで 12 |
| `"12"` / `"12px"` | 同じ |
| `"50%"` | 包含ブロックの半分 |
| `"auto"` | レイアウトアルゴリズムに任せる |

これ以外の文字列は、スタイルを組み立てるその瞬間に、何を期待していたかを書いたメッセージとともに panic します。`From<f32>` と `From<i32>` はポイントとして扱われます。数値リテラルに単位が要らないのはそのためです。

## アイテム属性（`ItemStyle`）

どの要素も受け取ります。

| 属性 | 型 | 意味 |
|---|---|---|
| `w` `h` | `Length` | 幅、高さ |
| `min_w` `min_h` | `Length` | 最小の幅、高さ |
| `max_w` `max_h` | `Length` | 最大の幅、高さ |
| `grow` | `f32` | `flex-grow`: 余った空間の取り分 |
| `shrink` | `f32` | `flex-shrink`: あふれた分の譲る割合 |
| `basis` | `Length` | `flex-basis`: 出発点の大きさ |
| `align_self` | `Align` | この子だけ親の `align` を上書きする |
| `m` `mx` `my` `mt` `mr` `mb` `ml` | `Length` | マージン: 全体、x、y、上、右、下、左 |
| `p` `px` `py` `pt` `pr` `pb` `pl` | `Length` | パディング。形は同じ |
| `col_span` `row_span` | `u16` | この子がまたぐグリッドのトラック数 |

マージンとパディングの短縮形は 全体 → 軸 → 辺 の順で、細かいほうが勝ちます。`p={8} pt={0}` は上以外が 8 ポイントです。

`min_w={0.0}` は名前で覚えておく価値があります。flex アイテムの自動的な最小サイズはその内容なので、幅の広い子はクリップされる代わりに隣を画面の外へ押し出します。譲るべきアイテムの最小サイズをゼロにするのが答えです。

## コンテナ属性（`ContainerStyle`）

`<View>` だけが受け取ります。

| 属性 | 型 | 値 |
|---|---|---|
| `display` | `Display` | `"flex"`（既定）、`"grid"`、`"block"`、`"none"` |
| `direction` | `Direction` | `"row"`（既定）、`"column"`、`"row-reverse"`、`"column-reverse"` |
| `wrap` | `bool` | 子を複数行に折り返す |
| `justify` | `Justify` | `"normal"`（既定）、`"start"`、`"end"`、`"flex-start"`、`"flex-end"`、`"center"`、`"stretch"`、`"space-between"`、`"space-evenly"`、`"space-around"` |
| `align` | `Align` | `"normal"`（既定）、`"start"`、`"end"`、`"flex-start"`、`"flex-end"`、`"center"`、`"baseline"`、`"stretch"` |
| `align_content` | `Justify` | 値は同じ。折り返した行を交差軸に分配する |
| `gap` | `Gap` | 両軸に同じ数値 1 つ、または `(column, row)` |
| `cols` | `u16` | 等幅のグリッド列。`display="grid"` のときだけ |

`justify` と `align` の既定は `Normal` です。これは「指定しない」という意味で、下のエンジン自身の既定をそのままにします。`"start"` とは違います。

不正な文字列は、受け付ける綴りの一覧とともに panic します。だから打ち間違いは、黙ってレイアウトが崩れる形ではなく、その要素を描く最初のフレームで表に出ます。

## 描画属性（`PaintStyle`）

`ItemStyle` の中に入っているので、これもどの要素も受け取ります。

| 属性 | 型 | 意味 |
|---|---|---|
| `bg` | `egui::Color32` | 背景。内容の後ろ |
| `border` | `egui::Stroke` | 枠線。内容の手前、ボックスの内側 |
| `radius` | `f32` | 角の丸み。背景・枠線・影で共有する |
| `shadow` | `bool` | テーマのウィンドウ影 |
| `custom_shadow` | `egui::Shadow` | 自前の影。`shadow` より優先される |
| `opacity` | `f32` | このノードが描くものすべての不透明度に掛ける |

3 つの図形はどれもノードのボーダーボックス、つまりボックス全体に乗ります。内容が描かれる矩形ではありません。**枠線はレイアウトでもあります**。その太さはパディングの縁に足されるので、子はストロークの内側から始まり、枠線はレイアウトが子を入れなかったその帯にちょうど描かれます。描画のほかの部分がレイアウトに届くことはありません。

描画はそのフレームのレイアウトが解けたあとに行われます。だから、子はそのままでコンテナだけが広がった場合も、同じパスの中で新しい大きさで描かれます。`<View>` の外（素の `Ui` モード）では `style` から何も描かれません。そこでの逃げ道は `<Frame>` です。

## `style=` と短縮形を一緒に使う

`style={expr}` と短縮形の属性は同じ prop を埋めます。両方あるときは、短縮形が式に連なります。

```rust
// .style((style).p(6)) になる
<Chip style={style} p={6}/>
```

これがあるので、ラッパーのコンポーネントは`#[prop(default)] style: ItemStyle` を取り、呼び出し側のレイアウトをそのまま受け取ったうえで、自分の分を足せます。

```rust
#[component]
fn Chip(cx: &mut Cx, #[prop(default)] style: ItemStyle, label: &str) {
    let clicked = cx.leaf(&style.shrink(0.0), |ui| ui.button(label).clicked());
    // ..
}
```

同じ型は手書きでも使えます。`ItemStyle::default().w(220.0).h(20.0)`、`ContainerStyle::default().direction("column")` のように。エスケープハッチが取るのもこれです。

## もっと知る

属性の完全な一覧、taffy への対応づけ、`<VirtualList>` の行の中で各属性がどうなるかは[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#layout-attributes)の 6 章にあります（英語）。
