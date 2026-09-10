# Bundled font for `examples/font`

`NotoSansJP-Subset.ttf` is a static Regular (wght=400) subset of Noto Sans JP,
compiled into the example as the `Bundled` font source. It is licensed under
the SIL Open Font License 1.1; the license text is in `OFL.txt` next to it.

| | |
|---|---|
| Declared family name | `Noto Sans JP` (name ID 1), style `Regular`, PostScript `NotoSansJP-Regular` |
| Size | 443,312 bytes (433 KB) |
| Glyphs | 1,937 (1,173 cmap entries) |
| Format | TrueType (`glyf`), static, no variation tables |
| SHA-256 | `79b0b56cc042932c6d914c3d293c95d7ebb42fcb56c397d2f1bf335ba8ccdda5` |

## Source

- URL: <https://raw.githubusercontent.com/google/fonts/main/ofl/notosansjp/NotoSansJP%5Bwght%5D.ttf>
  (variable font, `wght` 100–900, 9,589,900 bytes,
  SHA-256 `c2f3b4d463500a2ddcd3849cded1fceeb9fd6d1c32e6cbecd568453ba50fc68f`)
- Embedded version string: `Version 2.004-H2;hotconv 1.0.118;makeotfexe 2.5.65603`
- License: <https://raw.githubusercontent.com/google/fonts/main/ofl/notosansjp/OFL.txt>
  (copied verbatim to `OFL.txt`)
- Fetched from `main` on 2026-09-07. The exact google/fonts commit could not be
  recorded (GitHub's API and web UI were unreachable from the build
  environment); the version string above and the source SHA-256 identify the
  file.

## Coverage

- U+0020–007E Basic Latin (ASCII)
- U+00A0–00FF Latin-1 Supplement
- U+3000–303F CJK Symbols and Punctuation
- U+3040–309F Hiragana
- U+30A0–30FF Katakana
- U+FF00–FFEF Halfwidth and Fullwidth Forms
- 505 kanji: the 500 most frequent jōyō kanji (grades 1–6 and 8 in the
  KANJIDIC data, ranked by the KANJIDIC newspaper frequency field, ranks 1–500)
  plus the five sample-string kanji outside that set (吾 渋 猫 谷 輩)
- Every character of the example's five sample strings, checked by cmap lookup

Unassigned code points inside those blocks (for example U+3040, U+3097–3098,
U+FFBF–FFC1) have no glyph in the source font either.

## Regenerating

Requires Python 3 and `fonttools` (tested with fonttools 4.64.0, Python 3.11).
Run from a scratch directory:

```sh
python3 -m venv venv && venv/bin/pip install fonttools

curl -L -o 'NotoSansJP[wght].ttf' \
  'https://raw.githubusercontent.com/google/fonts/main/ofl/notosansjp/NotoSansJP%5Bwght%5D.ttf'
curl -L -o OFL.txt \
  'https://raw.githubusercontent.com/google/fonts/main/ofl/notosansjp/OFL.txt'

# 1. Static Regular instance (also rewrites the name table from Thin to Regular).
venv/bin/fonttools varLib.instancer --update-name-table \
  -o NotoSansJP-Regular-static.ttf 'NotoSansJP[wght].ttf' wght=400

# 2. Inputs for the subsetter.
printf '%s\n' 'U+0020-007E,U+00A0-00FF,U+3040-309F,U+30A0-30FF,U+3000-303F,U+FF00-FFEF' > unicodes.txt
printf '%s\n' \
  '日本語のテキストが表示できます。' \
  'フォントのフォールバックチェーン' \
  '吾輩は猫である。名前はまだ無い。' \
  '東京都渋谷区、2026年9月7日' \
  'egui-reactor で CSS の font-family のように書ける' > samples.txt
# kanji-list.txt: one line with the 505 kanji listed at the bottom of this file.

# 3. Subset. Output stays TTF (no --flavor); hinting and vertical metrics are kept.
venv/bin/pyftsubset NotoSansJP-Regular-static.ttf \
  --output-file=NotoSansJP-Subset.ttf \
  --unicodes-file=unicodes.txt \
  --text-file=kanji-list.txt \
  --text-file=samples.txt \
  --layout-features='*' \
  --name-IDs='*' \
  --notdef-outline
```

The kanji ranking came from the `freq` and `grade` fields of
<https://raw.githubusercontent.com/davidluzgouveia/kanji-data/master/kanji.json>
(a JSON export of KANJIDIC): keep entries with `grade` in {1,2,3,4,5,6,8}, sort
by `freq`, take the first 500, then union with the sample-string kanji.

### kanji-list.txt

```
一七万三上下不与世両中主乗九予争事二五井交京人今仕付代以件任企会伝位低住佐体何作使例供価係保信個側備働優元先党入全八公六共内円再写出分切初判別利制前副割力加助労動務勝勢化北区医十千午半協南原去参反収取受口可台各合同名向含吾告味呼命和品員商問営四回団国土在地型域基報場境増声売変外多夜大夫失女好始委姿子字学宅守安官定実宮害家容察審対導小少局展山岡島川州工差市席常平年幹広店府度建式引張強当形影役待後得復心必応念思急性情想意感態成戦所手打技投担持指挙提援撃支改放政教数整文料断新方施族日早明映昨時景書最月有望朝期木末本村条来東松果査校株核格案検業極楽構様権横機次欧止正武歳死残段毎比氏民気水求決沢治況法注活派流海消深済渉渋減渡港準演点無然物特状独猫率現球理環生産用田由申男町画界番疑病発白百的監目直相県真着知石研確示社神票福私移税種究空立第答策算米約終組経結統続総線置美義考者聞職育能脳自若英落葉蔵藤融衆行術衛表被裁補製西要見規視親観解言計記訪設訴証評試話認語説課調談論識警議護谷象負財費資賞質起足身車軍転輩輸農辺近述追退送通造連進運過道達違選部都配重野量金銀長門開間関閣防限院際集難電非面革韓音領頭題額食首験高鮮
```
