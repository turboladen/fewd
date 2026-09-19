#!/usr/bin/env bash
# PreToolUse(Bash) hook: before a command containing `git push` or `gh pr create`,
# run a local gate in the tree that command targets, and block it (exit 2) when
# the gate fails. The gate depends on the paths the outgoing commits and the
# working tree change: nothing runs when there are none or the push only deletes
# remote refs, `dprint check` and `typos` run when only markdown, LICENSE or
# .beads/ files change, and `just ci` runs for everything else. A command whose
# target tree or published ref the hook cannot check also exits 2. Any other
# failure exits 1, which Claude Code treats as non-blocking, so the hook fails
# open on its own errors. Bypass one call by prefixing each push segment with
# `SKIP_CI_HOOK=1`.

# This must run under macOS's bash 3.2. Errors are handled explicitly instead
# of through `set -e`, whose rules for subshells, functions and `||` would let
# a stray exit status of 2 block a push.
set -u -o pipefail

NL='
'

fail() {
  printf 'ci-before-push: hook error (%s); push NOT gated\n' "$1" >&2
  exit 1
}

note() {
  printf 'ci-before-push: %s\n' "$1" >&2
}

# Refuses a command whose published ref or target tree the hook cannot check.
# Exit 2 says "not verified", which a separate Bash call or the bypass answers.
block_unverifiable() {
  printf "ci-before-push: can't verify what this command publishes (%s); push the current branch from its own tree in a separate Bash call, or prefix SKIP_CI_HOOK=1.\n" "$1" >&2
  exit 2
}

trim() {
  local s=$1
  s=${s#"${s%%[![:space:]]*}"}
  s=${s%"${s##*[![:space:]]}"}
  printf '%s' "$s"
}

# Reads changed paths on stdin and prints the gate tier: none, light or full.
classify_changes() {
  local tier=none f
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    case "$f" in
      # dprint formats markdown and typos scans every tracked file, so even
      # these paths need the light gate. A .gitignore change alters what
      # eslint sees, so it is deliberately absent here and gets the full gate.
      *.md | LICENSE* | .git-blame-ignore-revs | .beads/*) tier=light ;;
      *)
        printf 'full\n'
        return 0
        ;;
    esac
  done
  printf '%s\n' "$tier"
}

run_gate() {
  if [ "$1" = light ]; then
    dprint check && typos --config .typos.toml
  else
    just ci
  fi
}

# Applies a `cd` that runs before the first push, outside a pipeline.
maybe_cd() {
  local seg=$1 arg target resolved
  [ "$push_seen" -eq 0 ] && [ "$piped" -eq 0 ] && [ -z "$unresolved_cd" ] || return 0
  case "$seg" in
    cd | "cd "*) ;;
    *) return 0 ;;
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
}

# Reads one push or PR-creation segment and records what it publishes.
parse_push_segment() {
  local seg=$1
  local w prev="" in_push=0 pending_value=0 remote_seen=0
  local deleting=0 refs_named=0 seg_refspecs="" words

  push_seen=1
  case "$seg" in
    "SKIP_CI_HOOK=1 git push"* | "SKIP_CI_HOOK=1 gh pr create"*) return 0 ;;
  esac
  all_bypassed=0

  read -r -a words <<<"$seg"

  # `gh pr create` publishes the branch named by --head, which need not be the
  # HEAD the diff below describes.
  case "$seg" in
    *"gh pr create"*)
      creates_pr=1
      gates_commits=1
      for w in "${words[@]}"; do
        if [ "$pending_value" -eq 1 ]; then
          pr_head=$w
          pending_value=0
        else
          case "$w" in
            --head | -H) pending_value=1 ;;
            --head=*) pr_head=${w#--head=} ;;
          esac
        fi
      done
      return 0
      ;;
  esac

  # Collect the refspecs of a `git push`: every positional word after the
  # remote, skipping redirections.
  for w in "${words[@]}"; do
    if [ "$in_push" -eq 0 ]; then
      [ "$w" = push ] && [ "$prev" = git ] && in_push=1
      prev=$w
      continue
    fi
    if [ "$pending_value" -eq 1 ]; then
      pending_value=0
      continue
    fi
    case "$w" in
      # These publish refs that a diff of HEAD cannot describe.
      --all | --mirror | --tags | --prune)
        [ -n "$unverifiable" ] || unverifiable="it publishes refs beyond the current branch"
        ;;
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
            # A lone colon is git's matching refspec; `:name` deletes a ref.
            :) [ -n "$unverifiable" ] || unverifiable="a matching refspec publishes every matching branch" ;;
            :*) ;;
            *)
              seg_refspecs="$seg_refspecs$w$NL"
              ;;
          esac
        fi
        ;;
    esac
  done

  # A segment that only deletes remote refs needs no gate. One without a
  # recognizable `git push` word pair, such as a quoted mention, always does.
  if [ "$in_push" -eq 0 ] || { [ "$deleting" -eq 0 ] && { [ "$refs_named" -eq 0 ] || [ -n "$seg_refspecs" ]; }; }; then
    gates_commits=1
    refspecs=$refspecs$seg_refspecs
  fi
  # What a push with no refspec publishes depends on push.default.
  [ "$in_push" -eq 1 ] && [ "$refs_named" -eq 0 ] && [ "$deleting" -eq 0 ] && bare_push=1
}

input=$(cat) || fail "could not read stdin"

# Coarse prefilter on the raw JSON, so most commands exit without spawning jq.
# It matches loosely, because the exact spelling only becomes visible once the
# command below is normalized.
case "$input" in
  *push* | *create*) ;;
  *) exit 0 ;;
esac

cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // ""') || fail "jq could not parse the hook input"
tool_cwd=$(printf '%s' "$input" | jq -r '.cwd // ""') || fail "jq could not parse the hook input"
if [ -z "$tool_cwd" ] || [ ! -d "$tool_cwd" ]; then
  tool_cwd=$(pwd) || fail "could not determine the working directory"
fi

# Join backslash continuations and squeeze runs of blanks, so detection and
# parsing see single spaces between tokens however the command was typed.
norm_cmd=$(printf '%s\n' "$cmd" |
  awk '{ if (sub(/\\$/, "")) printf "%s ", $0; else print }' |
  awk '{ gsub(/[ \t]+/, " "); sub(/^ /, ""); sub(/ $/, ""); print }') ||
  fail "could not normalize the command"

case "$norm_cmd" in
  *"git push"* | *"gh pr create"*) ;;
  *) exit 0 ;;
esac

# Split the command into simple commands on `&&`, `||`, `;` and newlines. Each
# piece of a pipeline gets a leading `|` marker, because a `cd` there runs in a
# subshell and does not move the push. The split ignores quoting, so a quoted
# separator splits too.
segments=$(printf '%s\n' "$norm_cmd" |
  awk '{ gsub(/\|\|/, "\n"); gsub(/&&/, "\n"); gsub(/;/, "\n"); print }' |
  awk '{ if (index($0, "|")) { n = split($0, p, "|"); for (i = 1; i <= n; i++) print "|" p[i] } else print }') ||
  fail "could not split the command"

run_dir=$tool_cwd
dir_stack=""
pending_closes=0
unresolved_cd=""
unverifiable=""
all_bypassed=1
gates_commits=0
creates_pr=0
pr_head=""
push_seen=0
bare_push=0
refspecs=""

while IFS= read -r seg; do
  # Apply the subshell exits of the previous segment: each `)` restores the
  # directory its `(` saved, after that segment's own `cd` has been read.
  while [ "$pending_closes" -gt 0 ] && [ -n "$dir_stack" ]; do
    run_dir=${dir_stack%%"$NL"*}
    dir_stack=${dir_stack#*"$NL"}
    pending_closes=$((pending_closes - 1))
  done
  pending_closes=0

  piped=0
  case "$seg" in
    "|"*)
      piped=1
      seg=${seg#|}
      ;;
  esac
  seg=$(trim "$seg")
  while [ "${seg#(}" != "$seg" ]; do
    dir_stack="$run_dir$NL$dir_stack"
    seg=$(trim "${seg#(}")
  done
  while [ "${seg%)}" != "$seg" ]; do
    pending_closes=$((pending_closes + 1))
    seg=$(trim "${seg%)}")
  done
  [ -n "$seg" ] || continue

  case "$seg" in
    *"git push"* | *"gh pr create"*) parse_push_segment "$seg" ;;
    *)
      # A branch change before the push means the branch this tree holds now is
      # not the one the push publishes.
      if [ "$push_seen" -eq 0 ] && [ "$piped" -eq 0 ] && [ -z "$unverifiable" ]; then
        case "$seg" in
          "git switch"* | "git checkout"*)
            case "$seg" in
              *" -- "*) ;;
              *) unverifiable="a branch change runs first" ;;
            esac
            ;;
        esac
      fi
      maybe_cd "$seg"
      ;;
  esac
done <<SEGMENTS
$segments
SEGMENTS

# Every push segment carries the bypass.
[ "$push_seen" -eq 1 ] && [ "$all_bypassed" -eq 1 ] && exit 0

# Without the target of a `cd`, the hook cannot tell which tree the push comes
# from, and validating any other tree could pass a branch nobody tested. Exit 2
# here means "not verified", and a standalone `cd` call avoids it.
if [ -n "$unresolved_cd" ]; then
  printf "ci-before-push: can't resolve '%s'; run the cd as its own Bash call, then push (or prefix SKIP_CI_HOOK=1).\n" "$unresolved_cd" >&2
  exit 2
fi

[ -n "$unverifiable" ] && block_unverifiable "$unverifiable"

# Every push segment only deletes remote refs.
[ "$push_seen" -eq 1 ] && [ "$gates_commits" -eq 0 ] && exit 0

if ! repo_root=$(git -C "$run_dir" rev-parse --show-toplevel 2>/dev/null); then
  note "$run_dir is not inside a git repository; not gating"
  exit 0
fi

branch=$(git -C "$repo_root" symbolic-ref --short -q HEAD 2>/dev/null) || branch=""
while IFS= read -r r; do
  [ -n "$r" ] || continue
  r=${r#+}
  [ "$r" = HEAD ] && continue
  # The diff below covers commits bound for the current branch's own name.
  if [ -n "$branch" ]; then
    case "$r" in
      "$branch" | "HEAD:$branch" | "HEAD:refs/heads/$branch" | "$branch:$branch" | "$branch:refs/heads/$branch") continue ;;
    esac
  fi
  block_unverifiable "the refspec '$r' names another branch"
done <<REFSPECS
$refspecs
REFSPECS

if [ -n "$pr_head" ] && [ "$pr_head" != "$branch" ]; then
  block_unverifiable "the PR head '$pr_head' names another branch"
fi

if [ "$bare_push" -eq 1 ]; then
  push_default=$(git -C "$repo_root" config push.default 2>/dev/null) || push_default=""
  [ "$push_default" = matching ] &&
    block_unverifiable "push.default is matching, so a bare push publishes every matching branch"
fi

# The diff runs from the merge-base with the upstream, or with origin/main for
# a first push. A PR's contents are everything since origin/main, even when the
# branch is already pushed. Uncommitted and untracked paths count too: the gate
# runs on the working tree, and a `git commit` earlier in the same command turns
# them into outgoing commits the diff cannot see yet. Any unknown state gets the
# full gate.
tier=full
base_ref=origin/main
if [ "$creates_pr" -eq 0 ]; then
  base_ref=$(git -C "$repo_root" rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null) || base_ref=origin/main
fi
base=$(git -C "$repo_root" merge-base HEAD "$base_ref" 2>/dev/null) || base=""
if [ -n "$base" ] &&
  committed=$(git -C "$repo_root" diff --name-only --no-renames "$base" HEAD 2>/dev/null) &&
  uncommitted=$(git -C "$repo_root" diff --name-only --no-renames HEAD 2>/dev/null) &&
  untracked=$(git -C "$repo_root" ls-files --others --exclude-standard 2>/dev/null); then
  tier=$(printf '%s\n%s\n%s\n' "$committed" "$uncommitted" "$untracked" | classify_changes) || fail "could not classify the changed paths"
fi

[ "$tier" = none ] && exit 0

gate_name="just ci"
[ "$tier" = light ] && gate_name="dprint check and typos"

if output=$(cd "$repo_root" && run_gate "$tier" </dev/null 2>&1); then
  exit 0
fi

{
  printf '%s failed in %s; refusing to run: %s\n' "$gate_name" "$repo_root" "$cmd"
  printf '\n--- last 25 lines of gate output ---\n'
  printf '%s\n' "$output" | tail -25
  printf '\nA fresh worktree needs dist/ and node_modules/ symlinked from the main checkout.\n'
  printf 'To bypass for one call: prefix the push with SKIP_CI_HOOK=1\n'
} >&2
exit 2
