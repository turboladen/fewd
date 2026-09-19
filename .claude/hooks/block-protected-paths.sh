#!/usr/bin/env bash
# PreToolUse hook for the editing tools: refuse agent edits to secrets, lock
# files, and SQLite databases, all of which change outside an editor.
#
# Hook input is JSON on stdin; NotebookEdit carries notebook_path in place of
# file_path. Paths arrive absolute or relative to the session's directory, so
# every rule compares the base name, case-insensitively because macOS resolves
# `.ENV` and `.env` to one file.
#
# Exit 2 blocks the tool call; a missing jq exits 1 to report the dead guard
# without blocking anything. Every other branch exits 0.

set -u
shopt -s nocasematch

if ! command -v jq >/dev/null 2>&1; then
  printf 'block-protected-paths: jq is missing, so protected paths are unguarded.\n' >&2
  exit 1
fi

file=$(jq -r '.tool_input.file_path // .tool_input.notebook_path // ""' 2>/dev/null) || exit 0
[ -n "$file" ] || exit 0

base=${file##*/}

case "$base" in
  .env.example|.env.sample) exit 0 ;;
esac

reason=""
case "$base" in
  .env|.env.*|*.env|.envrc) reason="Secrets belong in .env, which this repo does not edit through the agent; .env.example documents the keys." ;;
  bun.lock|bun.lockb|Cargo.lock) reason="Lock files change only by running bun or cargo." ;;
  *.db|*.db-shm|*.db-wal|*.db-journal) reason="Change the database through the app or a migration." ;;
esac

[ -n "$reason" ] || exit 0

printf 'BLOCKED: %s\n%s\n' "$file" "$reason" >&2
exit 2
