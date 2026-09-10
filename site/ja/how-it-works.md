---
title: 仕組み
---

# 仕組み

文法は React のもの、エンジンは egui のイミディエイトモードです。出会う違いのほとんどは、次の 4 つで説明がつきます。

## 何も保持しない

egui は毎フレーム、すべてのウィジェットを描き直します。差分を取る木はどこにもありません。`rsx!` も木を作りません。書いたその場で egui の呼び出しに展開されます。

だからハンドラは状態を `&mut` で借りられます。ハンドラはそのフレームの中で走り、すぐ捨てられるので、`'static` である必要がありません。

```rust
let mut count = use_state(cx, || 0i32);

rsx! {
    <View direction="row" gap={8}>
        <Button on_click={|| *count -= 1}>"-"</Button>
        <Button on_click={|| *count += 1}>"+"</Button>
    </View>
}
```

2 つのクロージャが `count` を順に可変で借ります。`Rc` も `RefCell` も`.clone()` もありません。

## 状態の鍵は位置

egui はスクロール位置や開閉フラグを `Context::Memory` に持ち、ウィジェットの木の中の位置から作った `Id` を鍵にします。フックも同じ考え方です。フックのスロットの鍵は **木の中の位置 + 呼び出し位置 + key** です。

つまり:

- `if` の中で `use_state` を呼べます。React の「毎回同じ順で呼ぶ」という規則は当てはまりません。
- 別の場所にある 2 つの `<Counter/>` は別々の状態を持ちます。
- ループの中の `<Counter key={i}/>` は key で区別されます。key が無いと、どの周回も同じスロットを叩きます。デバッグビルドではファイル名と行番号を赤いオーバーレイで表示します。
- コンポーネントを木の別の場所へ動かすと、その状態はリセットされます。
- `if show { <Counter/> }` が false になると、そのフレームでカウンタは訪問されません。状態は破棄され、`use_effect` のクリーンアップが走ります。これがアンマウントです。また true になれば、新しいマウントです。

## 毎フレーム、すべて読み直す

状態を購読するものはありません。どのコンポーネントも、必要なものを毎フレーム読みます。何も保持していない以上、細粒度の更新をしても得るものがありません。

「毎フレーム」は 60 fps という意味ではありません。egui が描き直すのは、入力があったときと、誰かが `request_repaint` を呼んだときです。状態が変わればランタイムが代わりに呼びます。`State` ガードの可変 deref、遅延書き込み、完了した future、`Dispatch` のメッセージ、どれでもです。

したがって **毎フレーム状態を書くコンポーネントは毎フレーム再描画され、アプリは休みません**。`TextEdit` のように毎フレーム書かざるを得ないウィジェットは`State::bind` を取ります。これは状態を「変わった」と印を付けずに `&mut T` を渡します。

描画そのものにはコストがあります。重い値は `use_memo` でキャッシュし、長いリストには `<VirtualList>` を使ってください。素の egui でするのと同じです。

## React と egui-reactor

| React | egui-reactor |
|---|---|
| 仮想 DOM、リコンサイラ | その場で展開、何も保持しない |
| `memo()`、`useCallback` | 不要。重い値には `use_memo` |
| エフェクトはコミット後に走る | `use_effect` はその場で走るので、ローカルを借りられる |
| フックは毎回同じ順で | フックの id は呼び出し位置から。`if` は自由、ループには `key` |
| `setState` は次のレンダーで見える | 書き込みは次のフレームで見える。描画済みのウィジェットは古い値のまま |
| deps の比較は `Object.is` | deps の比較は `Hash`。`(&str, &[T])` も使える |
| `<Provider value=..>` | 値を持つコンポーネントの中で `provide_context(cx, handle, children)` |
| Suspense は `throw` | `let Poll::Ready(x) = use_future(..) else { return };` |
| レンダーより長生きするコールバック | `Dispatch`。フレームをまたげる唯一のハンドル |

## どんなときに使うか

UI に構造があるときに使ってください。フォーム、パネル、自前の状態を持つリスト、流し直されるレイアウトなどです。[board](/ja/examples/board)、[form](/ja/examples/form)、[todo](/ja/examples/todo) は同じアプリを両方の書き方で見せます。行数はタブに出ています。

デバッグ用のパネルや `ui.label` を数行並べるだけなら、素の egui のほうが向いています。[list-10k](/ja/examples/list-10k) が分かれ目です。どちらの版もほぼ同じ長さになります。仕事をしているのが両方とも `ScrollArea::show_rows` だからです。

混ぜても構いません。`cx.leaf(&style, |ui| ..)` を使えば、木のどこからでも素のegui に降りられます。[エスケープハッチ](/ja/guide/escape-hatches) を見てください。

## もっと読む

いちばん小さいサンプルは [counter](/ja/examples/counter) です。[notes](/ja/examples/notes) はライブラリのほとんどを一度に使います。ランタイムの詳細は[docs/ARCHITECTURE.md](https://github.com/fand/egui-reactor/blob/main/docs/ARCHITECTURE.md)の 2 章と 5 章にあります（英語）。
