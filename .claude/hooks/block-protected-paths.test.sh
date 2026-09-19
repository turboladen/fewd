#!/usr/bin/env bash
# Drives block-protected-paths.sh with sample hook payloads and asserts the
# exit code for each: 2 means the guard blocks the edit, 0 means it allows one.
# Run it after changing either file. The paths below are matched as strings and
# need not exist.

set -u

dir=$(cd "$(dirname "$0")" && pwd)
hook="$dir/block-protected-paths.sh"
r=$(cd "$dir/../.." && pwd)
failures=0

# Asserts the exit code for one payload, named by label in the output.
expect_payload() {
  expected=$1
  label=$2
  payload=$3
  printf '%s' "$payload" | "$hook" >/dev/null 2>&1
  actual=$?
  if [ "$actual" = "$expected" ]; then
    printf 'ok    exit=%s  %s\n' "$actual" "$label"
  else
    printf 'FAIL  expected exit=%s, got exit=%s  %s\n' "$expected" "$actual" "$label"
    failures=$((failures + 1))
  fi
}

# Asserts the exit code for an Edit payload naming one path.
expect() {
  expect_payload "$1" "$2" \
    "$(printf '{"tool_name":"Edit","tool_input":{"file_path":"%s"}}' "$2")"
}

echo '-- secrets --'
expect 2 "$r/.env"
expect 2 "$r/.env.local"
expect 2 "$r/.env.production"
expect 2 "$r/server/.env"
expect 2 ".env"
expect 2 "./.env"
# dotenv tooling also names these files <stage>.env.
expect 2 "$r/production.env"
expect 2 "$r/config/app.env"
expect 2 "$r/.ENV"
# direnv keeps secrets in .envrc.
expect 2 "$r/.envrc"
expect 0 "$r/.env.example"
expect 0 ".env.example"
expect 0 "$r/.env.sample"
# A directory whose name starts with .env holds ordinary files.
expect 0 "$r/.env.d/local.ts"
expect 0 ".env.d/local.ts"

echo '-- lock files --'
expect 2 "$r/bun.lock"
expect 2 "$r/bun.lockb"
expect 2 "$r/Cargo.lock"
expect 2 "bun.lock"
expect 2 "Cargo.lock"
expect 0 "$r/src/lock.ts"

echo '-- database --'
expect 2 "$r/data/fewd.db"
expect 2 "$r/data/fewd.db-shm"
expect 2 "$r/data/fewd.db-wal"
expect 2 "$r/data/fewd.db-journal"
expect 2 "$r/.claude/worktrees/wt/data/fewd.db"
expect 2 "data/fewd.db"
expect 2 "./data/fewd.db"
# DATABASE_PATH names a file anywhere, so the rule matches the extension.
expect 2 "/tmp/scratch/fewd.db"
expect 2 "$r/fewd.DB"
expect 0 "$r/data/README.md"
expect 0 "$r/server/src/data/loader.rs"

echo '-- ordinary source files --'
expect 0 "$r/src/App.tsx"
expect 0 "$r/server/src/main.rs"
expect 0 "$r/package.json"
expect 0 "src/App.tsx"

echo '-- other payload shapes --'
expect_payload 2 'Write naming a lock file' \
  '{"tool_name":"Write","tool_input":{"file_path":"/r/Cargo.lock"}}'
expect_payload 2 'NotebookEdit naming a secret' \
  '{"tool_name":"NotebookEdit","tool_input":{"notebook_path":"/r/.env"}}'
expect_payload 0 'NotebookEdit naming a notebook' \
  '{"tool_name":"NotebookEdit","tool_input":{"notebook_path":"/r/analysis.ipynb"}}'
expect_payload 0 'payload without a path' '{"tool_name":"Edit","tool_input":{}}'
expect_payload 0 'malformed stdin' 'not json'
expect_payload 0 'empty stdin' ''

echo
if [ "$failures" -eq 0 ]; then
  echo 'All checks passed.'
else
  printf '%s check(s) failed.\n' "$failures"
  exit 1
fi
