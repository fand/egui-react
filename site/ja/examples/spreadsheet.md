---
title: spreadsheet
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="spreadsheet"
  :has-plain="data.spreadsheet.hasPlain"
  :react-lines="data.spreadsheet.reactLines"
  :plain-lines="data.spreadsheet.plainLines"
>

<template v-slot:summary>

26 × 10,000 セルの数式: メモ化を 2 段階、下書きは画面外にスクロールしても消えない。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source spreadsheet
:::

</div>

</template>

セルに数値を打てば、それを読んでいる合計がすべて動きます。数式を打てば、シートはもう一度パースされ、順序づけられ、評価されます。これは別々の仕事で、その2 つを区別することがこのサンプルの主題です。どのメッセージも`structure_rev`（どのセルが数式か、何を読むか、どういう順で評価できるか）か`value_rev`（変わったのは数値だけ）のどちらかを進め、それぞれに[`use_memo`](/ja/reference/hooks) が 1 つぶら下がります。ステータスバーにはそれぞれが何回走ったかが出るので、「リテラルのセルに 12 と打ってもシートはパースし直されない」ことを数字で見られます。

もう半分は、どの状態がアイテムのもので、どれが全体のものかという話です。そしてその問いを突きつけてくるのが [`<VirtualList>`](/ja/reference/elements) です。画面外にスクロールした行はアンマウントされ、そのフックは掃除されます。だから文書（セルのテキスト、列幅）は上のほうの `use_persisted` に置きます。選択、エディタ、その下書きも、同じく上のほうの reducer に置きます。スクロールしたら下書きが消えた、ではバグだからです。セルに残すのは、再マウントでリセット**されるべき** 状態だけです。「最初のフレームでフォーカスを取れ」「いま値が変わったから光れ」など。打ちかけの数式を画面の外へやって戻してみてください。ちゃんと残っています。

行のクロージャは、二度借りられないものを何も借りていません。2 つの memo の結果（どちらも `&'s` なので何とでも共存します）と `Copy` なスナップショットだけで、`State` ガードは 1 つもありません。セルがしたことはイベントとして上がり、行がそれをメッセージに変え、書き込むのは reducer だけです。固定されたヘッダは`<VirtualList on_scroll>` で本体に付いていきます。

## 自分で動かす

```sh
cargo run -p spreadsheet
trunk serve --config examples/spreadsheet/Trunk.toml
```

ソースは [`examples/spreadsheet/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/spreadsheet/src/lib.rs) です。

</ExamplePage>
