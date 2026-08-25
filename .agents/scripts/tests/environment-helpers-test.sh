#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "$0")" && pwd)
scripts_dir=$(cd "$script_dir/.." && pwd)

# shellcheck source=../lib/agent-context.sh
source "$scripts_dir/lib/agent-context.sh"
# shellcheck source=../lib/cloud-tools.sh
source "$scripts_dir/lib/cloud-tools.sh"

assert_eq() {
  expected=$1
  actual=$2
  message=$3
  if [ "$actual" != "$expected" ]; then
    echo "失敗: $message (expected: $expected, actual: $actual)" >&2
    exit 1
  fi
}

assert_success() {
  message=$1
  shift
  if ! "$@"; then
    echo "失敗: $message" >&2
    exit 1
  fi
}

assert_failure() {
  message=$1
  shift
  if "$@"; then
    echo "失敗: $message" >&2
    exit 1
  fi
}

original_path=$PATH
test_tmp=$(mktemp -d)
trap 'rm -rf "$test_tmp"' EXIT

unset CODEX_HOME CODEX_THREAD_ID CODEX_SESSION_ID CODEX_CI
assert_eq "claude" "$(agent_branch_prefix)" "Codex の印がない環境は claude を選ぶ"

CODEX_THREAD_ID=test-thread
assert_eq "codex" "$(agent_branch_prefix)" "Codex Desktop の thread ID で codex を選ぶ"
unset CODEX_THREAD_ID

CODEX_SESSION_ID=test-session
assert_eq "codex" "$(agent_branch_prefix)" "Codex の session ID で codex を選ぶ"
unset CODEX_SESSION_ID

CODEX_CI=1
assert_eq "codex" "$(agent_branch_prefix)" "Codex cloud / CI で codex を選ぶ"
unset CODEX_CI

CODEX_HOME="$test_tmp/codex-home"
assert_eq "codex" "$(agent_branch_prefix)" "従来の CODEX_HOME でも codex を選ぶ"
unset CODEX_HOME

client_bin="$test_tmp/client-bin"
mkdir -p "$client_bin"
touch "$client_bin/pg_isready"
chmod +x "$client_bin/pg_isready"
PATH="$client_bin:/usr/bin:/bin"
POSTGRESQL_INIT_SCRIPT="$test_tmp/missing-postgresql-service"
PATH="$client_bin" assert_failure "pg_isready だけでは PostgreSQL サーバーありと判定しない" postgresql_server_available

server_bin="$test_tmp/server-bin"
mkdir -p "$server_bin"
touch "$server_bin/postgres"
chmod +x "$server_bin/postgres"
PATH="$server_bin:/usr/bin:/bin"
PATH="$server_bin" assert_success "postgres コマンドがあれば PostgreSQL サーバーありと判定する" postgresql_server_available

installer_bin="$test_tmp/installer-bin"
cargo_home="$test_tmp/cargo-home"
mkdir -p "$installer_bin"
cat > "$installer_bin/curl" <<'CURL'
#!/usr/bin/env bash
set -euo pipefail
output=""
while [ $# -gt 0 ]; do
  if [ "$1" = "-o" ]; then
    shift
    output=$1
  fi
  shift
done
[ -n "$output" ]
cat > "$output" <<'INSTALLER'
#!/usr/bin/env bash
set -euo pipefail
mkdir -p "$CARGO_HOME/bin"
cat > "$CARGO_HOME/bin/rustup" <<'RUSTUP'
#!/usr/bin/env bash
exit 0
RUSTUP
chmod +x "$CARGO_HOME/bin/rustup"
INSTALLER
CURL
chmod +x "$installer_bin/curl"

PATH="$installer_bin:/usr/bin:/bin"
CARGO_HOME="$cargo_home"
export CARGO_HOME
assert_success "rustup がなければ公式インストーラーで導入する" ensure_rustup
assert_eq "$cargo_home/bin/rustup" "$(command -v rustup)" "導入した rustup に PATH を通す"

PATH=$original_path
echo "environment helpers: OK"
