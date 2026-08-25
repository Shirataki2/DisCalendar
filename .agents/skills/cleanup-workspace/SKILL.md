---
name: cleanup-workspace
description: DisCalendar の Git worktree、ローカルブランチ、ビルド成果物を安全に整理する。worktree の後片付け、マージ済みブランチの削除、ディスク容量不足や ENOSPC の解消を頼まれたときに使う。
---

# Cleanup workspace compatibility entrypoint

このリポジトリでは Claude Code と Codex が同じ手順を使えるよう、スキルの正本を
`.claude/skills/cleanup-workspace/` に置いている。

作業を始める前に [`../../../.claude/skills/cleanup-workspace/SKILL.md`](../../../.claude/skills/cleanup-workspace/SKILL.md) を最後まで読み、
そこから必要とされたスクリプトを使う。相対リンクは正本のディレクトリを基準に解決する。

