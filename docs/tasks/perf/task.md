# タスク: perf(レイアウトのフレームコスト、時期未定)

## 目的

react-egui のフレームコストを生 egui に近づける。今は同じ UI を react-egui で書くと生 egui より重く、web(120 Hz)では 1 フレームの予算 8.3 ms を超えて 16.7 ms に落ちる。原因はレイアウト層(egui_taffy)の 2 パス制と、要素ごとの固定コストにある。

## 症状(2026-09、examples PR B の list-10k で観測)

- web で `<VirtualList>` をスクロールすると、スクロール中の全フレームで `egui PERF WARNING: request_discard has been called N frames in a row` が出る。fps 表示も乱れる。
- スクロールしていないあいだも react-egui 側は 16.7 ms(60 Hz 相当)、生 egui 側は 8.3 ms(120 Hz)。desktop でも react-egui の方が遅い。
- kittest の CPU 計測(`examples/list-10k/tests/bench.rs`、release、600×800、20 フレーム):

| rows | `<ScrollArea>` + `for` | `<VirtualList>` | 生 egui `show_rows` |
|---|---|---|---|
| 100 | 0.88 ms | 0.36 ms | 0.16 ms |
| 1,000 | 5.06 ms | 0.28 ms | 0.13 ms |
| 10,000 | 78.04 ms | 0.27 ms | 0.17 ms |

`<VirtualList>` で行数依存は消えたが、画面内の十数行だけでも生 egui の 2 倍かかる。

## 原因(egui_taffy 0.14 のソースから)

1. **レイアウトが 2 パス制。** egui_taffy は「子を描いてサイズを測る → taffy を計算する → 結果が前回と違えば `request_discard` で同じフレームをもう 1 回描く」(`egui_taffy/src/lib.rs` 632 行: `taffy.dirty(node) || state.last_size != root_rect.size()` のとき再計算、712 行で `request_discard`)。dirty になる条件は、新しいノード、測定サイズの変化、ルートのサイズ変化。生 egui は 1 パス。
2. **新規ツリーは必ず discard する。** 測定値が無い状態から始まるので、初回は常に dirty。`<VirtualList>` の行は `<View>` で行ごとに小さい taffy ツリーを作るため、スクロールで行が入れ替わるたびに新規ツリーが生まれ、毎フレーム discard が走る。これが PERF WARNING の直接原因。
3. **要素ごとの固定コスト。** 要素 1 つ = taffy ノード 1 つ + `Ui::push_id` の子 `Ui` + Id のハッシュ + ストア参照。ノード数に比例してかかり、wasm では native の 2〜3 倍。
4. **アイドル時の dirty(未確認)。** 測定サイズが浮動小数のゆらぎで毎フレーム変われば、静止中も毎フレーム 2 パスになる。計測で確定させる。

関連する既知の性質(examples plan.md 7〜8 章): taffy の leaf は「前回描いたサイズ」を min / max 両方の content size として返す。初回は幅 0 の `Ui` で描かれる。`grow` は余白の分配であってサイズではない。

## スコープ

### 含む

- **計測**(最初にやる。結果を本書に書く)
  - native / web で、`ctx.will_discard()` と pass 数を毎フレーム数える計測フックを runner に仮置きする(`Options` の debug フラグでよい)。
  - gallery の list-10k(`for` / `VirtualList` / plain)、showcase、counter で、アイドル時とスクロール中の pass 数とフレーム時間を取る。web は Chrome の Performance パネル。
  - アイドル時に discard が起きているかを確定させる(原因 4)。
- **egui_taffy 側の改善**(fork して試作 → 上流 PR)
  - 描かずに測る: テキスト系 leaf は `ui.fonts(|f| f.layout(..))` でサイズが取れる。描画前に測定できる leaf だけ先に測り、初回から 1 パスで確定させる。
  - 親がサイズを渡している新規ツリー(`leaf_fill`、`VirtualList` の行)は discard せずその場で確定させる。
  - 測定値を丸めて dirty 判定を安定させる(原因 4 が確定した場合)。
- **react-egui 側の改善**
  - `VirtualList` の行が taffy ツリーを作らずに済む道(行の内側だけ egui の `horizontal` で組む `Row` 要素、または行を 1 つの taffy ツリーの子ノードとして再利用する)。
  - `Cx::leaf` / `container` の固定コスト(子 `Ui` の生成、Id ハッシュ)のプロファイルと削減。
  - ランナーの `max_passes` の既定値の見直し。
- 計測を `examples/list-10k/tests/bench.rs` と同じ形で残し、改善の前後を数字で比べる。

### 含まない

- egui 本体の変更。
- レイアウトエンジンの置き換え(egui_flex 等)。ARCHITECTURE 6 章で却下済み。
- wgpu / 描画側の最適化(描画は egui のもの)。

## 成果物

- 計測結果(本書に表で追記)。
- egui_taffy への PR(または fork の差分と、上流に出せない理由)。
- react-egui の変更と、その前後の bench。
- ARCHITECTURE.md 5.3(多重パス)、6 章の更新。

## 終了条件

- web(120 Hz)の gallery で、list-10k の `VirtualList` をスクロールしても PERF WARNING が出ず、アイドル時に 8.3 ms に収まる。
- 上の表の `<VirtualList>` 列が生 egui の 1.5 倍以内。
- 既存テストと snapshot がすべて通る(snapshot が変わる場合は理由を plan に書く)。

## 決めごと(着手時点での前提)

- 先に計測、次に egui_taffy、最後に react-egui 側。原因を確定させる前に react-egui 側を触らない。
- egui_taffy の変更は上流に出す前提で書く。react-egui の main に fork を依存として入れない。
- 優先度はフェーズ 8(公開準備)の前後。examples PR C(wgpu)より後。
