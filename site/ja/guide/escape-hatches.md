---
title: エスケープハッチ
---

# エスケープハッチ

egui-reactor が包んでいるのは egui の一部分です。それで構いません。最後は必ず`&mut egui::Ui` に行き着くので、egui にできることは呼び出し 1 回の距離にあります。`egui-reactor-elements` の要素も、同じ呼び出しで書かれています。

| やりたいこと | 使うもの |
|---|---|
| 木の途中に普通のコードを書く | `{view(\|cx\| ..)}` |
| いまいる場所に egui を描く | `cx.leaf(&style, \|ui\| ..)` |
| 自分で描画する | `<Canvas>`、または矩形を確保して描く葉 |
| egui コンテナのクロージャの中でフックを使う | 内側の `Ui` に対する新しい `Cx` |

## 唯一の落とし穴

`cx.ui()` はどこからでも安全に読めます。visuals、style、input など。ただし`<View>` の中では、それは木が始まった場所の `Ui` であって、いまいる場所ではありません。これを通して描くと、ウィジェットは木の左上隅に出ます。

`cx.leaf` は矩形を流れの中に置き、その矩形の `Ui` を渡します。描くときは必ずこれを使ってください。

## 1. クロージャ

```rust
rsx! {
    <View direction="column" gap={8}>
        <Text>"above"</Text>
        {view(|cx| {
            // ここでもフックは使える。同じ `Cx` で、スロットの鍵はこの行。
            let mut opened = use_state(cx, || 0u32);
            let dark = cx.ui().visuals().dark_mode;

            cx.leaf(&ItemStyle::default(), |ui| {
                ui.horizontal(|ui| {
                    ui.small(if dark { "dark" } else { "light" });
                    if ui.link("open the docs").clicked() {
                        *opened += 1;
                    }
                });
            });
        })}
    </View>
}
```

## 2. 葉

専用の要素が無いウィジェットのために使います。葉はレイアウトのノードなので、同じスタイル属性を取ります。

```rust
{view(move |cx| {
    cx.leaf(&ItemStyle::default(), |ui| {
        ui.color_edit_button_srgba(colour.bind());
    });
})}
```

`cx.leaf` は描いたものを測って、その大きさを申告します。プログレスバーやスクロール領域のように、与えられた分だけ広がるウィジェットには`cx.leaf_fill` を使い、大きさを渡してください。渡さないと、全部を取ります。

```rust
{view(move |cx| {
    // 両方の軸に大きさが要る。広がる葉の軸を `auto` にすると、
    // ウィンドウ全体を取ってしまう。
    cx.leaf_fill(&ItemStyle::default().w(220.0).h(20.0), |ui| {
        ui.add(egui::ProgressBar::new(value).show_percentage());
    });
})}
```

## 3. ペインタ

`<Canvas>` は矩形を確保して渡してくれます。

```rust
rsx! {
    <Canvas grow={1.0} paint={|ui: &mut egui::Ui, rect: egui::Rect| {
        ui.painter().rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    }}/>
}
```

同じ矩形が wgpu のコールバックの置き場所にもなります(`egui_wgpu::Callback::new_paint_callback(rect, ..)`)。[shader](/ja/examples/shader) はこれでフラグメントシェーダを画面に出しています。`sense` の既定は `Sense::hover()` です。`on_drag` には `Sense::drag()` が要ります。

手で書くなら、広がる葉になります。

```rust
fn sparkline(ui: &mut egui::Ui, values: &[f32]) {
    let (rect, _response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    // ..
}
```

この関数は素の egui です。ライブラリのことを何も知りません。

## 4. 入れ子の `Cx`

egui のコンテナのクロージャの中では、新しい `Ui` が手に入ります。そこに新しい`Cx` を組めば、またフックが使えます。

```rust
let (store, scope) = (cx.store, cx.scope_id());

rsx! {
    {view(move |cx| {
        cx.leaf(&ItemStyle::default(), move |ui| {
            ui.group(|ui| {
                let mut cx = Cx::new(store, ui, scope);
                cx.scope("inner", |cx| {
                    let mut inner = use_state(cx, || 0i32);
                    let value = *inner;
                    let ui = cx.ui();
                    ui.horizontal(|ui| {
                        if ui.button("inner +").clicked() {
                            *inner += 1;
                        }
                        ui.label(format!("inner: {value}"));
                    });
                });
            });
        });
    })}
}
```

`cx.scope("inner", ..)` が入れ子の木に専用の id を与えるので、そのフックが外側のフックとぶつかりません。

## 木の中に素の egui の画面を丸ごと置く

その画面の状態はフック 1 つに持たせ、広がる葉の中に描きます。ギャラリーが各サンプルの「素の egui」版を動かしているのも、この形です。

```rust
#[component]
fn PlainScreen(cx: &mut Cx) {
    let mut state = use_state(cx, plain::PlainState::default);
    cx.leaf_fill(&ItemStyle::default().grow(1.0), |ui| {
        plain::ui(ui, state.bind());
    });
}
```

`&mut *state` ではなく `bind()` を使ってください。素の `ui` は毎フレーム状態に書き込むので、`&mut *state` だと毎フレーム再描画を要求し続けてしまいます。

## 動かして見る

[escape-hatch](/ja/examples/escape-hatch) はこの 4 つを 1 画面に並べたものです。[shader](/ja/examples/shader) と [patch](/ja/examples/patch) は、ペインタ経由でwgpu に行きます。
