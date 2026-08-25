#!/usr/bin/env bash

postgresql_server_available() {
  local postgres_bindir=""
  local init_script=${POSTGRESQL_INIT_SCRIPT:-/etc/init.d/postgresql}

  command -v postgres >/dev/null 2>&1 && return 0
  command -v pg_ctlcluster >/dev/null 2>&1 && return 0
  [ -x "$init_script" ] && return 0

  if command -v pg_config >/dev/null 2>&1; then
    postgres_bindir=$(pg_config --bindir 2>/dev/null || true)
    [ -n "$postgres_bindir" ] && [ -x "$postgres_bindir/postgres" ] && return 0
  fi

  return 1
}

ensure_rustup() {
  local cargo_home=""
  local installer=""

  command -v rustup >/dev/null 2>&1 && return 0
  command -v curl >/dev/null 2>&1 || return 1

  if [ -n "${CARGO_HOME:-}" ]; then
    cargo_home=$CARGO_HOME
  elif [ -n "${HOME:-}" ]; then
    cargo_home="$HOME/.cargo"
  else
    return 1
  fi

  installer=$(mktemp) || return 1
  if ! curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o "$installer"; then
    rm -f "$installer"
    return 1
  fi

  export CARGO_HOME=$cargo_home
  if ! sh "$installer" -y; then
    rm -f "$installer"
    return 1
  fi
  rm -f "$installer"

  export PATH="$CARGO_HOME/bin:$PATH"
  command -v rustup >/dev/null 2>&1
}
