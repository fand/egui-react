// The Japanese one-line summaries for the example cards.
//
// The English ones live in each example's `META` on disk, next to the code
// they describe (`example-source.js`, `summary`). A Rust source file has no
// place for a second language, so the translations live here, keyed by the
// example's directory name. `summaryJa` throws when a name is missing, the way
// `summary` throws when a `META` has none: a card never comes out blank, and
// an example added without its translation fails the site build.

const SUMMARIES_JA = {
  notes: 'メモアプリ: reducer、永続化、コンテキスト、メモ化、エディタをまとめて。',
  board: 'カードをドラッグして列の間を移動しても、入力中のタイトルはそのまま残る。',
  patch: '自分で WGSL シェーダを生成し、検証し、プレビューするノードエディタ。',
  spreadsheet:
    '26 × 10,000 セルの数式: メモ化を 2 段階、下書きは画面外にスクロールしても消えない。',
  counter: '1 つの状態を、3 つのハンドラが順に借りる。',
  todo: 'reducer がリストを動かし、`use_persisted` が再起動をまたいで保持する。',
  form: '束縛できるウィジェットを全部、変更ログ付きで。設定は再起動しても残る。',
  theme: '一番上で渡した 2 つの値を、3 階層下で読む。間のコンポーネントは何もしない。',
  clock: '自分で再描画を要求するストップウォッチと、後片付けをするエフェクト。',
  'custom-hook': '自作フック 3 つを、それぞれ独自の状態を持つ 2 つのコンポーネントから呼ぶ。',
  'escape-hatch':
    '素の egui へ降りる 4 つの道: クロージャ、リーフ、ペインタ、入れ子の Cx。',
  shader: '`<Canvas>` の中でレイトレースしたブラックホール。スライダーが uniform につながる。',
  'list-10k': '1 万行を、全部描くとどれだけかかるか。',
  layout: '`<View>` が解釈する flex と grid の属性を、1 節につき 1 つずつ。',
  styles: '`style` 属性を表に全部: 名前、使うコード、描かれる姿。',
  fetch: '`use_future` がリクエストを走らせ、一番近い `<Suspense>` がスピナーを描く。',
  font: 'CSS 風のフォントチェーン: 同梱・取得・インストール済みのフォントと、各項目の解決先。'
}

/** The Japanese summary for an example, by directory name. */
export function summaryJa(name) {
  const text = SUMMARIES_JA[name]
  if (!text) {
    throw new Error(`summaries-ja: no Japanese summary for "${name}". Add one in site/lib/summaries-ja.js.`)
  }
  return text
}
