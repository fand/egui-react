---
title: アーキテクチャ
---

# アーキテクチャ

<script setup>
// `withBase` を使うのは、このサイトが GitHub Pages では `/egui-reactor/`、
// プレビューでは `/` から配信されるため。`/api/` は cargo doc の出力で、
// このサイトのページではないので、ルータのルートではなく素のリンクにする。
import { withBase } from 'vitepress'
</script>

このサイトがドキュメントです。設計のメモはリポジトリに置いてあります。説明している当のコードの隣にあり、コードと一緒にレビューされるからです。このページは、それがどこにあるかを書いたものです。設計のメモはいまのところ英語だけです。

## `docs/ARCHITECTURE.md`

[docs/ARCHITECTURE.md](https://github.com/fand/egui-reactor/blob/main/docs/ARCHITECTURE.md)が深いほうのリファレンスで、実装について「いま何が本当か」を書いています。目標と非目標、`Cx` と `View` トレイト、`#[component]` と `rsx!` が生成するもの、フック一覧とその正確な意味、ランタイム（ストア、掃除、マルチパス、再描画の方針、`<Suspense>`）、taffy の上のレイアウトエンジンとリストの行のための軽量パス、要素一覧、クレート構成、プラットフォームの注意、テスト戦略。

このサイトのガイドはアプリを作る人に向けて書いてあります。ARCHITECTURE はライブラリを変える人に向けたものです。ガイドのページが何かを要約するときは、繰り返さずに向こうの節へリンクします。

## `docs/adr/`

[docs/adr/](https://github.com/fand/egui-reactor/tree/main/docs/adr) は決定の記録です。設計上の決定 1 つにつき 1 ファイルで、ドメインごとに分けてあります。`core`、`runtime`、`layout`、`elements`、`fonts`、`a11y`、`app`。それぞれに、いつ決めたか、何を決めたか、何を退けたか、その理由、そして何を代償にしたかが書いてあります。

規律はわざと狭くしてあります。この記録を読む価値のあるものにしているのが、それです。

- ADR はある瞬間の記録です。別のことを言うように書き直すことはありません。
- 新しい決定には新しいファイルを作ります。決定が **変わった** ときも新しいファイルを作り、古いほうを supersede します。古いファイルは本文をそのまま保ち、ステータス行に "superseded by" が付きます。
- ARCHITECTURE.md はいま何が本当かを書き、ADR にリンクします。両者が食い違うときは、コードについて間違っているのは ARCHITECTURE のほうです。

だから、`use_state` が `(値, セッタ)` の組ではなくガードを返す理由、フックのdeps を比較ではなくハッシュにした理由、`on_*` の props を 1 つのクロージャにまとめた理由、レイアウトを egui_flex ではなく taffy に載せた理由が知りたければ、答えは日付の入った短いファイルにあります。しかもそれは、書かれてから編集されていません。

## API ドキュメント

<a :href="withBase('/api/')" target="_blank" rel="noreferrer">/api/</a> は公開している 4 クレート ― `egui-reactor`、`egui-reactor-elements`、`egui-reactor-app`、`egui-reactor-macros` ― の `cargo doc` です。このサイトと同じビルドで生成されるので、つねにサイトをビルドしたコミットと一致します。正確なシグネチャを見るならそちら、部品どうしの噛み合い方を知るならこのサイトです。

## リポジトリ

[github.com/fand/egui-reactor](https://github.com/fand/egui-reactor)。Issue もプルリクエストも歓迎します。出す前に走らせるテストは README にあります。
