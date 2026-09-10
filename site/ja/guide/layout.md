---
title: レイアウト
---

# レイアウト

レイアウトは属性としての Flexbox と Grid です。`<View>` は[taffy](https://github.com/DioxusLabs/taffy) の上に組んだ小さなレイアウトエンジンのノードで、葉は egui のウィジェットです。

```rust
rsx! {
    <View direction="row" justify="space-between" align="center" gap={8} p={12}>
        <Text grow={1.0}>"Title"</Text>
        <Button on_click={|| *open = true}>"Open"</Button>
    </View>
}
```

`<View>` の中の `<View>` は同じ木に加わります。素の `egui::Ui` の中の`<View>` は、新しい木を始めます。

## 属性

コンテナ属性は `<View>` に付けて、その子を配置します。アイテム属性はどの要素にでも付けられ、その要素を親の中に配置します。一覧は[レイアウト属性](/ja/reference/layout-attributes) にあります。

**サイズ**: `w` `h` `min_w` `min_h` `max_w` `max_h`。数値は egui のポイントです。文字列は `"auto"`、`"50%"`、`"12px"`、`"12"` が書けます。

```rust
<View w="100%" max_w={720.0} min_h={0.0}>..</View>
```

**Flex（コンテナ）**: `direction`（`"row"`、`"column"`、`"row-reverse"`、`"column-reverse"`）、`wrap`、`justify`（主軸）、`align`（交差軸）、`align_content`。

**Flex（アイテム）**: `grow`、`shrink`、`basis`、`align_self`。

```rust
<View direction="row" gap={8}>
    <View w={200.0} shrink={0.0}>"sidebar"</View>
    <View grow={1.0} min_w={0.0}>"the rest"</View>
</View>
```

譲るべき列には `min_w={0.0}` を付けてください。付けないと、幅の広い子はクリップされる代わりに隣を画面の外へ押し出します。

**間隔**: コンテナには `gap`（数値 1 つ、または `(column, row)`）。アイテムにはマージンとパディング: `m` `mx` `my` `mt` `mr` `mb` `ml`、`p` `px` `py` `pt` `pr` `pb` `pl`。より細かい指定が勝ちます。

**Grid**: `display="grid"` と `cols={n}` で、等幅の列が `n` 本できます。`col_span` と `row_span` で子を広げます。

```rust
<View display="grid" cols={3} gap={8}>
    <Text col_span={2}>"wide"</Text>
    <Text>"narrow"</Text>
</View>
```

## `<Text>` と `<Label>`

どちらもテキストを描きます。`<Text>` はレイアウトのノードで、`size`、`color`、`strong`、`selectable`、`font` の props を持ちます。`<Label>` は葉に入った`egui::Label` です。`<View>` の中では `<Text>` を使ってください。

どちらも既定では折り返しません。だから flex の行の中では、テキストは自分の全幅を申告します。折り返すなら `wrap` を渡し、折り返す先の幅も渡します。

```rust
<Text wrap w="100%">{long_paragraph}</Text>
```

## ボックスを描く

どの要素もボックスを描けます。`bg`、`border`、`radius`、`shadow`、`custom_shadow`、`opacity`。

```rust
<View p={12} gap={8} bg={egui::Color32::from_gray(30)} radius={8.0} shadow>
    <Text>"a card"</Text>
</View>
```

影と背景は内容の後ろ、枠線は手前に描かれ、どれも同じ `radius` に従います。枠線はパディングに足されるので、子はストロークの内側から始まります。

すでにボックスを描くウィジェット（`<Button>`、`<TextEdit>`、`<ComboBox>`）は、こちらがボックスを描くと自分のボックスを譲ります。二重にはなりません。`<Button>` に付けた `p` は、ボタン自身のパディングになります。

`<View>` の外では、`style` は何も描きません。そこでは `<Frame>` を使います。

## つまずきどころ 3 つ

### `ScrollArea` には高さが要る

`ScrollArea` は与えられた分だけ広がるので、レイアウトがその「与える分」を知っている必要があります。高さの決まったコンテナの中で `grow` を付けるか、自分で高さを持たせてください。

```rust
<View direction="column" grow={1.0} min_h={0.0}>
    <Text strong>"header"</Text>
    <ScrollArea grow={1.0}>
        {rows}
    </ScrollArea>
</View>
```

高さの決まっていない列の中では `grow` だけでは足りません。スクロール領域がウィンドウの高さ全部を要求し、画面がウィンドウより高くなってしまいます。

### 折り返しには幅が要る

`<Text>`、`<Label>`、`<View>` の `wrap` には幅が要ります。`w` と組み合わせるか、幅の決まった親の下で `grow` と組み合わせてください。

### `display="none"` は `if` ではない

```rust
// 隠れているだけでマウントは続く: フックは走り続け、状態も残る。
<View display={if showing { "flex" } else { "none" }}>
    <Preview/>
</View>

// アンマウント: 状態は破棄され、エフェクトのクリーンアップが走る。
if showing {
    <Preview/>
}
```

`display="none"` は部分木のボックスをゼロにし、支援技術からも隠しますが、木には残します。状態を保ったままにしたいペインに使ってください。状態を消してよいなら`if` を使います。

## 動かして見る

[layout](/ja/examples/layout) は flex と grid の属性を全部たどります。[styles](/ja/examples/styles) は描画側で同じことをします。[board](/ja/examples/board) はそれらで組んだ実際の画面です。エンジンの説明は[docs/ARCHITECTURE.md](https://github.com/fand/egui-reactor/blob/main/docs/ARCHITECTURE.md#6-layout)の 6 章にあります（英語）。
