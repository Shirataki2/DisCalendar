<!-- タイトルは squash マージ時のコミットメッセージになるので、変更内容が分かる一文にする (例: サーバー設定ダイアログを実装) -->

## 概要

<!-- 何を・なぜ変えたか (1〜3 行) -->

## 関連 Issue

Closes #

## 変更内容

-

## 動作確認

<!-- 確認した手順や追加したテスト。UI の変更はスクリーンショットがあると分かりやすい -->

- [ ]

## チェックリスト

- [ ] web: `pnpm lint` / `pnpm exec tsc --noEmit` / `pnpm build` が通る (CI でも確認される)
- [ ] api / bot: `cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` が通る
- [ ] sqlx のクエリを追加・変更した場合は該当クレートで `cargo sqlx prepare -- --all-targets` を実行して `.sqlx/` を更新した
- [ ] DB スキーマを変える場合は新しいマイグレーションで行い、api / bot 両方の読み書きを揃えた (api/README.md)
- [ ] README / docs を更新した (必要な場合)
- [ ] `.env*` や秘密情報 (トークン・鍵) を含めていない
- [ ] Claude / Codex のレビュー指摘を確認し、対応したか理由をコメントした
