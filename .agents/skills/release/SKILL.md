---
name: release
description: DisCalendar の共通バージョン決定、リリース準備、タグ、GitHub Release、本番デプロイ、ロールバックを行う。リリース、バージョン更新、タグ、本番反映を頼まれたときに使う。
---

# Release compatibility entrypoint

このリポジトリでは Claude Code と Codex が同じ手順を使えるよう、スキルの正本を
`.claude/skills/release/` に置いている。

作業を始める前に [`../../../.claude/skills/release/SKILL.md`](../../../.claude/skills/release/SKILL.md) を最後まで読み、
そこから必要とされたスクリプトを使う。相対リンクは正本のディレクトリを基準に解決する。

