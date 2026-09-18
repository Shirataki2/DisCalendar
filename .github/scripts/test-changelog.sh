#!/usr/bin/env bash
# 一時リポジトリで集約とリリースノートを検証する。実際の版・履歴には触れない。
set -euo pipefail
root=$(git rev-parse --show-toplevel)
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture"/{.github/scripts,.agents/scripts/lib,.claude/skills/release/scripts,web/src/content,api,bot,changelog.d}
cp "$root"/.github/scripts/*.sh "$fixture/.github/scripts/"
cp "$root/.agents/scripts/lib/agent-context.sh" "$fixture/.agents/scripts/lib/"
cp "$root/.claude/skills/release/scripts/bump-version.sh" "$fixture/.claude/skills/release/scripts/"
cd "$fixture"
git init -q
printf '{\n  "version": "3.0.0"\n}\n' > web/package.json
for crate in api bot; do
  printf '[package]\nname = "discalendar-%s"\nversion = "3.0.0"\n' "$crate" > "$crate/Cargo.toml"
  printf '[[package]]\nname = "discalendar-%s"\nversion = "3.0.0"\n' "$crate" >> Cargo.lock
done
cat > web/src/content/changelog.mdx <<'HISTORY'
{/* コメント */}

前書き

### 既存の未リリース記事

- 既存の変更

## v3.0.0 (2026年9月1日)

### 公開済みの記事

- [使い方](/docs/gettingstarted)
HISTORY
reject() {
  if "$@"; then
    echo "失敗すべき操作が成功しました: $*" >&2
    exit 1
  fi
}
notes() { bash .github/scripts/release-notes.sh "$@"; }
bump() { bash .claude/skills/release/scripts/bump-version.sh "$1" 2026-09-18 > /dev/null 2>&1; }
notes v3.0.0 > old-notes
printf '# 書き方\n' > changelog.d/README.md
printf '### 変更A\n\n- [リンク](/docs/a)\n' > changelog.d/249-a.md
# ファイル末尾に改行がなくても、次の記事との境界を保つ
printf '### 変更B\n\n- 変更Bの内容' > changelog.d/251-b.md
notes > pending
grep -q '^### 変更A$' pending
grep -q '^### 変更B$' pending
grep -q '^### 既存の未リリース記事$' pending
grep -q 'https://discalendar.app/docs/a' pending
reject grep -q '公開済み\|書き方\|前書き' pending
notes v3.1.0-rc.1 > prerelease
diff -u pending prerelease
bump 3.1.0-rc.1
test -f changelog.d/249-a.md
reject grep -q '^## v3.1.0' web/src/content/changelog.mdx
bump 3.1.0
notes v3.1.0 > released
diff -u pending released
test ! -f changelog.d/249-a.md
test ! -f changelog.d/251-b.md
test -f changelog.d/README.md
grep -q '^## v3.1.0 (2026年9月18日)$' web/src/content/changelog.mdx
notes v3.0.0 > old-notes-after
diff -u old-notes old-notes-after
cp web/src/content/changelog.mdx released-history
reject bump 3.1.0
diff -u released-history web/src/content/changelog.mdx
test -z "$(notes)"
bump 3.1.1
diff -u released-history web/src/content/changelog.mdx
# 個別ファイルのみの場合も、直近の公開済み見出しの前に集約する
printf '### 変更C\n\n- 内容\n' > changelog.d/260-c.md
bump 3.1.2
test "$(notes v3.1.2)" = $'### 変更C\n\n- 内容'
# 壊れた個別ファイルはバージョン更新前に拒否し、元のファイルを保持する
printf '## v9.0.0\n' > changelog.d/270-invalid.md
reject notes 2> /dev/null
reject bump 3.1.3
test -f changelog.d/270-invalid.md
bash .github/scripts/check-versions.sh 3.1.2 > /dev/null 2>&1
# 並行PRが別ファイルを追加する場合、マージと集約で競合・欠落しない。
rm changelog.d/270-invalid.md
git config user.name 'Test'
git config user.email 'test@example.invalid'
git config commit.gpgsign false
git add .
git commit -qm 'テスト初期状態'
git checkout -qb feature-a
printf '### 並行A\n\n- A\n' > changelog.d/280-a.md
git add changelog.d/280-a.md
git commit -qm '記事A'
git checkout -qb feature-b HEAD~1
printf '### 並行B\n\n- B\n' > changelog.d/281-b.md
git add changelog.d/281-b.md
git commit -qm '記事B'
git merge --no-edit feature-a > /dev/null
notes > parallel
bump 3.2.0
notes v3.2.0 > parallel-released
diff -u parallel parallel-released
test "$(grep -c '^### 並行' parallel-released)" -eq 2
printf '更新履歴のテスト成功\n'
