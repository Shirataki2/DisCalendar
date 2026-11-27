#!/usr/bin/env bash

is_codex_session() {
  [ -n "${CODEX_THREAD_ID:-}" ] ||
    [ -n "${CODEX_SESSION_ID:-}" ] ||
    [ -n "${CODEX_CI:-}" ]
}

is_codex_agent() {
  is_codex_session || [ -n "${CODEX_HOME:-}" ]
}

is_gemini_agent() {
  [ -n "${GEMINI_CLI:-}" ] ||
    [ -n "${GEMINI_SESSION_ID:-}" ] ||
    [ -n "${GEMINI_PROJECT_DIR:-}" ] ||
    [ -n "${ANTIGRAVITY_AGENT:-}" ] ||
    [ -n "${ANTIGRAVITY_CONVERSATION_ID:-}" ]
}

agent_branch_prefix() {
  if is_codex_session; then
    echo "codex"
  elif is_gemini_agent; then
    echo "gemini"
  elif is_codex_agent; then
    echo "codex"
  else
    echo "claude"
  fi
}
