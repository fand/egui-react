---
title: コンポーネントとイベント
---

# コンポーネントとイベント

## `#[component]`

コンポーネントは、第 1 引数が `cx: &mut Cx` の関数です。残りの引数はすべてprops です。

```rust
#[component]
fn Card(cx: &mut Cx, title: &str, count: i32) {
    rsx! {
        <View direction="column" gap={4} p={8}>
            <Text strong>{title}</Text>
            <Text>{format!("{count}")}</Text>
        </View>
    }
}
```

`<Card title="inbox" count={3}/>` のように使います。マクロがビルダー付きのprops 構造体を作るので、prop の渡し忘れはコンパイルエラーになります。

描かれるのは本体の末尾の式です。分岐は 1 つの `rsx!` の中に入れてください。分岐ごとに `rsx!` を書いてはいけません。`rsx!` はそれぞれ別のクロージャ型です。

```rust
rsx! { if open { <Body/> } else { <Placeholder/> } }
```

## Props

| 書き方 | 意味 |
|---|---|
| `title: &str` | 必須 |
| `label: Option<&str>` | 省略可。渡さなければ `None` |
| `#[prop(default)] style: ItemStyle` | 省略可。渡さなければ `Default::default()` |
| `#[prop(default = true)] enabled: bool` | 省略可、既定値つき |
| `#[prop(default, into)] direction: Direction` | セッタが `impl Into<Direction>` を取るので `direction="row"` と書ける |
| `children: impl View` | 子ノード |

`children` はつねに存在します。宣言しなければ `()` で、その要素は空でなければいけません。部分木を受け取るなら `children: impl View`、`<Button>` のようにテキストを受け取るなら `children: impl Into<egui::WidgetText>` と宣言します。

```rust
#[component]
fn Panelled(cx: &mut Cx, #[prop(default)] style: ItemStyle, children: impl View) {
    rsx! {
        <View style={style} direction="column" gap={8} p={12} bg={egui::Color32::DARK_GRAY}>
            {children}
        </View>
    }
}
```

呼び出し側が配置するコンポーネントには、`#[prop(default)] style: ItemStyle` を取らせてください。`rsx!` が呼び出し側の `w`、`grow`、`p` などから埋めるので、`<Panelled grow={1.0}/>` はほかの要素と同じようにレイアウトされます。

## イベント

prop に `#[event]` を付けると、それはエミッタになります。型がペイロードです。

```rust
#[component]
fn Dialog(cx: &mut Cx, title: &str, #[event] on_ok: (), #[event] on_cancel: ()) {
    rsx! {
        <View direction="column" gap={8}>
            <Text strong>{title}</Text>
            <View direction="row" gap={8}>
                <Button on_click={|| on_ok.emit(())}>"OK"</Button>
                <Button on_click={|| on_cancel.emit(())}>"Cancel"</Button>
            </View>
        </View>
    }
}
```

呼び出し側はこう書きます。

```rust
<Dialog title="Quit?" on_ok={|| *open = false} on_cancel={|| *open = false}/>
```

どちらのハンドラも `open` を `&mut` で借ります。これが通るのは、`rsx!` が1 つの要素の `on_*` をすべて 1 つのクロージャにまとめ、生成されたイベント enumで match するからです。ハンドラはペイロードを取っても無視しても構いません。`on_change={|v: bool| ..}` と `on_change={|| ..}` はどちらも通ります。

### イベント enum はスコープに要る

まとめられたクロージャは `DialogEvent::Ok` という名前を書くので、`Dialog` と一緒に `DialogEvent` も import してください。elements の prelude は`ButtonEvent` や `TextEditEvent` などをすでに export しています。

```rust
use crate::components::{Dialog, DialogEvent};
```

イベント名を書き間違えるとコンパイルが通りません。

### `events=`

1 か所でまとめて扱いたいときは、クロージャを 1 つ渡して自分で match します。

```rust
<Dialog title="Quit?" events={|e| match e {
    DialogEvent::Ok(_) => *open = false,
    DialogEvent::Cancel(_) => {}
}}/>
```

`rsx!` が `on_*` 属性から生成しているのは、これです。

### ペイロードは借用できる

`#[event] on_change: &str` と書けます。生成された enum がライフタイムを持つので、`<TextEdit on_submit={|text: String| ..}>` のような形でも、必要以上に確保しません。

## `shares_ui`

要素はふつう、自分専用の子 `egui::Ui` を持ちます。egui のコンテナにはそれで困るものがあります。ドッキングされたパネルは親の `Ui` から場所を切り取りますし、`Grid` は行が変わるたびに `Ui` を書き換えます。`#[component(shares_ui)]` は、フックのスコープは 1 段深いままにしつつ、描画は親の `Ui` に対して行います。`Panel`、`CentralPanel`、`Row`、`Suspense` がこれを使っています。必要になるのはそういうコンテナを包むときだけです。

## 動かして見る

[form](/ja/examples/form) は束縛できるウィジェット全部に対する props とイベントです。[theme](/ja/examples/theme) はプロバイダのコンポーネント、[notes](/ja/examples/notes) はコンポーネント・イベント・reducer をまとめて使います。生成されるコードの説明は[docs/ARCHITECTURE.md](https://github.com/fand/egui-reactor/blob/main/docs/ARCHITECTURE.md#33-components)の 3.3 節と 3.6 節にあります（英語）。
