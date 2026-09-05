# 共有予定の OGP 用フォント

Google Fonts の [Noto Sans JP](https://github.com/google/fonts/tree/main/ofl/notosansjp) を
ウェイト 500 に固定し、WOFF に変換して同梱しています (SIL Open Font License 1.1、隣の `NotoSansJP-OFL.txt`)。
日本語の予定を描画する際に、予定の文字列を外部のフォント配信サービスへ送らないために使用します。

元ファイル: `NotoSansJP[wght].ttf` (2026-09-05 取得)。変換には fontTools の
`instantiateVariableFont(font, {"wght": 500}, inplace=True)` と `font.flavor = "woff"` を使用しました。
