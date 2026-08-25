#!/usr/bin/env bash

is_codex_agent() {
  [ -n "${CODEX_HOME:-}" ] ||
    [ -n "${CODEX_THREAD_ID:-}" ] ||
    [ -n "${CODEX_SESSION_ID:-}" ] ||
    [ -n "${CODEX_CI:-}" ]
}

agent_branch_prefix() {
  if is_codex_agent; then
    echo "codex"
  else
    echo "claude"
  fi
}
