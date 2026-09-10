---
title: rsx!
---

# rsx!

`rsx! { .. }` は `impl View` を返します。中身は、表示されるときに描くクロージャです。マクロは書いたその場で egui の呼び出しに展開されるので、制御構文は普通の`if` と `for` です。

## 要素

```rust
rsx! {
    <View direction="row" gap={8} align="center">
        <Text strong>"Title"</Text>
        <Button on_click={|| *open = true}>"Open"</Button>
    </View>
}
```

どの要素も Rust の関数コンポーネントです。`<Button/>` を書くには `Button` がスコープに要ります。HTML のタグはありません。パスも書けます: `<elements::Separator vertical/>`。

いつもの import:

```rust
use egui_reactor::prelude::*;
use egui_reactor_elements::prelude::*;
```

## テキストと式

テキストは文字列リテラルでなければいけません。裸の単語はコンパイルエラーです。

```rust
rsx! {
    <Text>"a literal"</Text>
    <Text>{format!("{} items", items.len())}</Text>
    <Text>{name.as_str()}</Text>
}
```

`{expr}` には `View` であるものなら何でも書けます。`&str`、`String`、`Option<V>`、`Vec<V>`、配列、別の `rsx!`、`&mut Cx` を取るクロージャ。

## 属性

名前で見分ける 3 種類があります。

- `on_*={handler}` はイベントです。[コンポーネントとイベント](/ja/guide/components-and-events) を見てください。
- レイアウトと描画の属性（`w`、`h`、`grow`、`p`、`m`、`bg`、`border`、`radius`、…）は、その要素の `style` prop に入ります。どの要素も受け取ります。一覧は [レイアウト属性](/ja/reference/layout-attributes) にあります。
- それ以外はすべて、そのコンポーネントの prop です。

値を書かない属性は `true` です。

```rust
rsx! {
    <Text strong wrap>"a strong, wrapping label"</Text>
    <Separator vertical/>
}
```

レイアウトの値は数値でも CSS 風の文字列でも書けます。

```rust
rsx! {
    <View w={200.0} p={12} gap={8}>
        <Text w="50%">"half"</Text>
    </View>
}
```

`style={expr}` と短縮形は連結されます。`<Chip style={style} p={6}/>` は`.style((style).p(6))` になります。ラッパーコンポーネントは、呼び出し側のレイアウトを受け取って足せます。

## 子

タグの間にあるものが子です。子が無ければ `()` です。リテラルか `{expr}` が1 つだけなら、その式そのものが渡されます。だから`<Button>"OK"</Button>`（子は `impl Into<WidgetText>`）と`<View>..</View>`（子は `impl View`）が同じ見た目になります。

## 制御構文

`rsx!` の中では `if`、`else`、`for`、`match` が使えます。

```rust
rsx! {
    <View direction="column" gap={4}>
        if todos.is_empty() {
            <Text>"nothing to do"</Text>
        } else {
            for (i, todo) in todos.iter().enumerate() {
                <View key={i} direction="row" gap={8}>
                    <Text>{todo.text.as_str()}</Text>
                    <Button on_click={|| dispatch.send(Msg::Remove(i))}>"x"</Button>
                </View>
            }
        }
        match status {
            Status::Idle => { <Text>"idle"</Text> }
            Status::Busy(n) => { <Text>{format!("{n} left")}</Text> }
        }
    </View>
}
```

`match` のアームの本体には波かっこが要ります。アームが持つのは式ではなく要素だからです。

`items.iter().map(|i| rsx!{..})` は要りませんし、その形が起こす借用エラーも出てきません。

## `key`

`key={expr}` は要素の id に混ぜられます。`for` の中では、フックを持つ要素には必ず要ります。無いと、どの周回も同じスロットを叩き、衝突検出が文句を言います。

```rust
for card in cards.iter() {
    <Card key={card.id} card={card}/>
}
```

key は `Hash + Debug` である必要があります。key を変えると、その部分木はリセットされます。古い id は破棄され、新しい id が新規にマウントされます。

## 素の egui に降りる

`{view(|cx| ..)}` は、木の途中に置く普通のコードです。

```rust
rsx! {
    <View direction="column" gap={8}>
        <Text>"above"</Text>
        {view(|cx| {
            cx.leaf(&ItemStyle::default(), |ui| {
                ui.color_edit_button_srgba(colour.bind());
            });
        })}
    </View>
}
```

同じ `Cx` なので、この中でもフックは使えます。描くときは `cx.ui()` ではなく`cx.leaf` を通してください。[エスケープハッチ](/ja/guide/escape-hatches) を見てください。

## 動かして見る

[layout](/ja/examples/layout) と [styles](/ja/examples/styles) は属性を 1 つずつ見せます。[notes](/ja/examples/notes) は文法をひととおり使います。マクロの規則は[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#32-view-and-rsx)の 3.2 節にあります（英語）。
