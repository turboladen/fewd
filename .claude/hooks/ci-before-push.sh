#!/usr/bin/env bash
# PreToolUse(Bash) hook: before a command containing `git push` or `gh pr create`,
# run a local gate in the tree that command targets, and block it (exit 2) when
# the gate fails. scripts/changed-scopes.mjs maps the paths the outgoing commits
# and the working tree change to scopes, and verify.mjs runs the CI gates those
# scopes select; nothing runs when nothing changed or the push only deletes
# remote refs. A tree without the classifier, or a machine without bun, runs
# `just ci`. Any other failure exits 1, which Claude Code treats as
# non-blocking, so the hook fails open on its own errors. Bypass one call by
# prefixing each push segment with `SKIP_CI_HOOK=1`.

# This must run under macOS's bash 3.2. Errors are handled explicitly instead
# of through `set -e`, whose rules for subshells, functions and `||` would let
# a stray exit status of 2 block a push.
set -u -o pipefail

fail() {
  printf 'ci-before-push: hook error (%s); push NOT gated\n' "$1" >&2
  exit 1
}

note() {
  printf 'ci-before-push: %s\n' "$1" >&2
}

trim() {
  local s=$1
  s=${s#"${s%%[![:space:]]*}"}
  s=${s%"${s##*[![:space:]]}"}
  printf '%s' "$s"
}

input=$(cat) || fail "could not read stdin"

# Coarse prefilter on the raw JSON, so most commands exit without spawning jq.
case "$input" in
  *"git push"* | *"gh pr create"*) ;;
  *) exit 0 ;;
esac

cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // ""') || fail "jq could not parse the hook input"
tool_cwd=$(printf '%s' "$input" | jq -r '.cwd // ""') || fail "jq could not parse the hook input"
if [ -z "$tool_cwd" ] || [ ! -d "$tool_cwd" ]; then
  tool_cwd=$(pwd) || fail "could not determine the working directory"
fi

case "$cmd" in
  *"git push"* | *"gh pr create"*) ;;
  *) exit 0 ;;
esac

# Split the command into simple commands on `&&`, `||`, `;` and newlines, after
# joining backslash continuations. Each piece of a pipeline gets a leading `|`
# marker, because a `cd` there runs in a subshell and does not move the push.
# The split ignores quoting, so a quoted separator splits too.
segments=$(printf '%s\n' "$cmd" |
  awk '{ if (sub(/\\$/, "")) printf "%s ", $0; else print }' |
  awk '{ gsub(/\|\|/, "\n"); gsub(/&&/, "\n"); gsub(/;/, "\n"); print }' |
  awk '{ if (index($0, "|")) { n = split($0, p, "|"); for (i = 1; i <= n; i++) print "|" p[i] } else print }') ||
  fail "could not split the command"

run_dir=$tool_cwd
unresolved_cd=""
force_full=0
gates_commits=0
creates_pr=0
push_seen=0
refspecs=""

while IFS= read -r seg; do
  piped=0
  case "$seg" in
    "|"*)
      piped=1
      seg=${seg#|}
      ;;
  esac
  seg=$(trim "$seg")
  while [ "${seg#(}" != "$seg" ]; do
    seg=$(trim "${seg#(}")
  done
  while [ "${seg%)}" != "$seg" ]; do
    seg=$(trim "${seg%)}")
  done
  [ -n "$seg" ] || continue

  case "$seg" in
    *"git push"* | *"gh pr create"*) ;;
    *)
      # Only a `cd` that runs before the first push, outside a pipeline, moves
      # the push into another tree.
      [ "$push_seen" -eq 0 ] && [ "$piped" -eq 0 ] && [ -z "$unresolved_cd" ] || continue
      case "$seg" in
        cd | "cd "*) ;;
        *) continue ;;
      esac
      arg=$(trim "${seg#cd}")
      case "$arg" in
        \"*\") arg=${arg#\"} && arg=${arg%\"} ;;
        \'*\') arg=${arg#\'} && arg=${arg%\'} ;;
      esac
      case "$arg" in
        \~) arg=${HOME:-} ;;
        \~/*) arg=${HOME:-}/${arg#\~/} ;;
      esac
      target=""
      case "$arg" in
        "" | -* | *'$'* | *'`'*) ;;
        /*) target=$arg ;;
        *) target=$run_dir/$arg ;;
      esac
      if [ -n "$target" ] && [ -d "$target" ] && resolved=$(cd "$target" 2>/dev/null && pwd); then
        run_dir=$resolved
      else
        unresolved_cd=$seg
      fi
      continue
      ;;
  esac

  push_seen=1
  case "$seg" in
    "SKIP_CI_HOOK=1 git push"* | "SKIP_CI_HOOK=1 gh pr create"*) continue ;;
  esac
  case "$seg" in
    *"gh pr create"*) creates_pr=1 ;;
  esac

  # Collect the refspecs of a `git push`: every positional word after the
  # remote, skipping redirections. Flags that push refs other than HEAD force
  # the full gate. A deletion pushes no commits, so it adds no refspec.
  read -r -a words <<<"$seg"
  prev=""
  in_push=0
  pending_value=0
  remote_seen=0
  deleting=0
  refs_named=0
  seg_refspecs=""
  for w in "${words[@]}"; do
    if [ "$in_push" -eq 0 ]; then
      [ "$w" = push ] && [ "${prev:-}" = git ] && in_push=1
      prev=$w
      continue
    fi
    if [ "$pending_value" -eq 1 ]; then
      pending_value=0
      continue
    fi
    case "$w" in
      --all | --mirror | --tags | --prune) force_full=1 ;;
      --delete | -d) deleting=1 ;;
      --repo)
        pending_value=1
        remote_seen=1
        ;;
      --repo=*) remote_seen=1 ;;
      -o | --push-option | --receive-pack | --exec) pending_value=1 ;;
      # A bare redirection operator takes the next word as its target.
      *'>' | *'<') pending_value=1 ;;
      *'>'* | *'<'* | '&') ;;
      -*) ;;
      *)
        if [ "$remote_seen" -eq 0 ]; then
          remote_seen=1
        else
          refs_named=1
          case "$w" in
            :*) ;;
            *)
              seg_refspecs="$seg_refspecs$w
"
              ;;
          esac
        fi
        ;;
    esac
  done

  # A segment that only deletes remote refs needs no gate. One without a
  # recognizable `git push` word pair, such as `gh pr create`, always does.
  if [ "$in_push" -eq 0 ] || { [ "$deleting" -eq 0 ] && { [ "$refs_named" -eq 0 ] || [ -n "$seg_refspecs" ]; }; }; then
    gates_commits=1
    refspecs=$refspecs$seg_refspecs
  fi
done <<SEGMENTS
$segments
SEGMENTS

# Every push segment is bypassed or only deletes remote refs.
[ "$push_seen" -eq 1 ] && [ "$gates_commits" -eq 0 ] && exit 0

# Without the target of a `cd`, the hook cannot tell which tree the push comes
# from, and validating any other tree could pass a branch nobody tested. Exit 2
# here means "not verified", and a standalone `cd` call avoids it.
if [ -n "$unresolved_cd" ]; then
  printf "ci-before-push: can't resolve '%s'; run the cd as its own Bash call, then push (or prefix SKIP_CI_HOOK=1).\n" "$unresolved_cd" >&2
  exit 2
fi

if ! repo_root=$(git -C "$run_dir" rev-parse --show-toplevel 2>/dev/null); then
  note "$run_dir is not inside a git repository; not gating"
  exit 0
fi

branch=$(git -C "$repo_root" symbolic-ref --short -q HEAD 2>/dev/null) || branch=""
while IFS= read -r r; do
  [ -n "$r" ] || continue
  r=${r#+}
  [ "$r" = HEAD ] && continue
  # The diff only covers commits bound for the current branch's own name.
  if [ -n "$branch" ]; then
    case "$r" in
      "$branch" | "HEAD:$branch" | "HEAD:refs/heads/$branch" | "$branch:$branch" | "$branch:refs/heads/$branch") continue ;;
    esac
  fi
  force_full=1
done <<REFSPECS
$refspecs
REFSPECS

# The diff runs from the upstream when HEAD contains it. A first push, a PR, and
# a force push that rewrites pushed commits diff from the merge-base with
# origin/main instead: a PR's contents are everything since origin/main, and a
# diff from a rewritten upstream misses the paths of the commits it drops.
# Uncommitted and untracked paths count too, because a `git commit` earlier in
# the same command pushes them. Any unknown state gets every gate.
#
# The classifier runs inside the target tree, so its own copy decides. A tree
# without one predates it, and its verify.mjs rejects --scope, so it gets
# `just ci`, as does a machine without bun.
if ! command -v bun >/dev/null 2>&1 || [ ! -f "$repo_root/scripts/changed-scopes.mjs" ]; then
  gate_name="just ci"
  output=$(cd "$repo_root" && just ci </dev/null 2>&1) && exit 0
else
  run_all=1
  scope_args=()
  if [ "$force_full" -eq 0 ]; then
    base_ref=origin/main
    if [ "$creates_pr" -eq 0 ] &&
      upstream=$(git -C "$repo_root" rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null) &&
      git -C "$repo_root" merge-base --is-ancestor "$upstream" HEAD 2>/dev/null; then
      base_ref=$upstream
    fi
    # The classifier prints `none` for an empty diff and `all` for a base it
    # cannot diff against. A failure, no output, or an unknown line selects
    # every gate.
    if listed=$(cd "$repo_root" && bun scripts/changed-scopes.mjs --base "$base_ref" --format lines </dev/null 2>/dev/null) &&
      [ -n "$listed" ]; then
      run_all=0
      while IFS= read -r scope; do
        [ -n "$scope" ] || continue
        case "$scope" in
          none) ;;
          docs | frontend | rust) scope_args+=(--scope "$scope") ;;
          *) run_all=1 ;;
        esac
      done <<LISTED
$listed
LISTED
      [ "$run_all" -eq 0 ] && [ "${#scope_args[@]}" -eq 0 ] && exit 0
    fi
  fi
  [ "$run_all" -eq 1 ] && scope_args=(--all)
  gate_name="verify.mjs ${scope_args[*]}"
  output=$(cd "$repo_root" && bun .claude/skills/verify/verify.mjs --ci-only --fast --skip lockfile ${scope_args[@]+"${scope_args[@]}"} </dev/null 2>&1) && exit 0
fi

{
  printf '%s failed in %s; refusing to run: %s\n' "$gate_name" "$repo_root" "$cmd"
  printf '\n--- last 60 lines of gate output ---\n'
  printf '%s\n' "$output" | tail -60
  printf '\nA fresh worktree needs dist/ and node_modules/ symlinked from the main checkout.\n'
  printf 'To bypass for one call: prefix the push with SKIP_CI_HOOK=1\n'
} >&2
exit 2
