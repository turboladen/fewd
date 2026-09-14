#!/usr/bin/env bash
# PreToolUse(Bash) hook: before a command that runs `git push` or `gh pr create`,
# run a local gate in the tree that command targets, and block it (exit 2) when
# the gate fails. scripts/changed-scopes.mjs maps the paths the outgoing commits
# and the working tree change to scopes, and verify.mjs runs the CI gates those
# scopes select; nothing runs when nothing changed or the push only deletes
# remote refs. A tree without the classifier, or a machine without bun, runs
# `just ci`, and a command whose target tree or published ref the hook cannot
# check exits 2. Any other failure exits 1, so the hook fails open on its own
# errors. Bypass one call by prefixing each push segment with `SKIP_CI_HOOK=1`.

# This must run under macOS's bash 3.2, where `"${a[@]}"` on an empty array is
# an unbound-variable error, so every array expansion is length guarded. Errors
# are handled explicitly instead of through `set -e`, whose rules for subshells,
# functions and `||` would let a stray exit status of 2 block a push.
set -u -o pipefail

NL='
'

# Stands where `||` did, so a `cd` the shell may skip is distinguishable from
# one it always runs.
OR_MARK='@@ci-before-push:or@@'

fail() {
  printf 'ci-before-push: hook error (%s); push NOT gated\n' "$1" >&2
  exit 1
}

# Refuses a command whose published ref or target tree the hook cannot check.
# Exit 2 says "not verified", which a separate Bash call or the bypass answers.
block_unverifiable() {
  printf "ci-before-push: can't verify what this command publishes (%s); push the current branch from its own tree in a separate Bash call, or prefix SKIP_CI_HOOK=1.\n" "$1" >&2
  exit 2
}

# Records the first reason, because it is the one the reader can act on.
mark_unverifiable() {
  [ -n "$unverifiable" ] || unverifiable=$1
}

trim() {
  local s=$1
  s=${s#"${s%%[![:space:]]*}"}
  s=${s%"${s##*[![:space:]]}"}
  printf '%s' "$s"
}

# Applies a `cd` that runs before the first push, outside a pipeline.
maybe_cd() {
  local seg=$1 arg target resolved
  [ "$push_seen" -eq 0 ] && [ "$piped" -eq 0 ] && [ -z "$unresolved_cd" ] || return 0
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

# Splits one simple command into its leading environment assignments, the
# command word, that word's global options, and the subcommand, so detection
# follows the shell's own reading rather than a substring. Sets seg_cmd,
# seg_sub, seg_sub2, seg_args, seg_bypass and seg_override; returns 1 when the
# command word is neither git nor gh.
scan_segment() {
  local seg=$1 w pending="" wrapper=0 i=0 n
  local words
  seg_cmd=""
  seg_sub=""
  seg_sub2=""
  seg_args=()
  seg_bypass=0
  seg_override=""

  read -r -a words <<<"$seg"
  n=${#words[@]}

  # Environment assignments, and an `env` wrapper around them, precede the
  # command word. The git variables among them point git at another repository.
  while [ "$i" -lt "$n" ]; do
    w=${words[$i]}
    case "$w" in
      SKIP_CI_HOOK=1) seg_bypass=1 ;;
      GIT_DIR=* | GIT_WORK_TREE=* | GIT_COMMON_DIR=* | GIT_OBJECT_DIRECTORY=* | GIT_INDEX_FILE=*)
        [ -n "$seg_override" ] || seg_override="the command sets ${w%%=*}"
        ;;
      [A-Za-z_]*=*) ;;
      env | */env) wrapper=1 ;;
      -i | --ignore-environment)
        [ "$wrapper" -eq 1 ] || break
        ;;
      -u)
        [ "$wrapper" -eq 1 ] || break
        i=$((i + 1))
        ;;
      *) break ;;
    esac
    i=$((i + 1))
  done

  [ "$i" -lt "$n" ] || return 1
  case "${words[$i]}" in
    git | */git) seg_cmd=git ;;
    gh | */gh) seg_cmd=gh ;;
    *) return 1 ;;
  esac
  i=$((i + 1))

  # Global options sit between the command word and its subcommand. Those that
  # name another repository or tree, or set configuration, decide what a push
  # publishes, so the tree the hook can see says nothing about it.
  while [ "$i" -lt "$n" ]; do
    w=${words[$i]}
    i=$((i + 1))
    if [ -n "$pending" ]; then
      pending=""
      continue
    fi
    case "$w" in
      -C | --git-dir | --work-tree | --namespace | -c | --config-env)
        [ -n "$seg_override" ] || seg_override="the command passes $w"
        pending=1
        ;;
      --git-dir=* | --work-tree=* | --namespace=* | --config-env=*)
        [ -n "$seg_override" ] || seg_override="the command passes ${w%%=*}"
        ;;
      -R | --repo | --exec-path | --super-prefix) pending=1 ;;
      -*) ;;
      *)
        seg_sub=$w
        break
        ;;
    esac
  done
  [ -n "$seg_sub" ] || return 1

  # `gh pr create` names its action in two words.
  if [ "$seg_cmd" = gh ] && [ "$seg_sub" = pr ]; then
    while [ "$i" -lt "$n" ]; do
      w=${words[$i]}
      i=$((i + 1))
      case "$w" in
        -*) ;;
        *)
          seg_sub2=$w
          break
          ;;
      esac
    done
  fi

  while [ "$i" -lt "$n" ]; do
    seg_args[${#seg_args[@]}]=${words[$i]}
    i=$((i + 1))
  done
  return 0
}

# Records what a `git push` publishes, reading the words after `push`.
parse_push_segment() {
  local w i=0 pending_value=0 remote_seen=0 deleting=0 refs_named=0 seg_refspecs=""

  while [ "$i" -lt "${#seg_args[@]}" ]; do
    w=${seg_args[$i]}
    i=$((i + 1))
    if [ "$pending_value" -eq 1 ]; then
      pending_value=0
      continue
    fi
    case "$w" in
      # These publish refs that a diff of HEAD cannot describe.
      --all | --mirror | --tags | --prune)
        mark_unverifiable "it publishes refs beyond the current branch"
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
            :) mark_unverifiable "a matching refspec publishes every matching branch" ;;
            :*) ;;
            *) seg_refspecs="$seg_refspecs$w$NL" ;;
          esac
        fi
        ;;
    esac
  done

  # A segment that only deletes remote refs needs no gate.
  if [ "$deleting" -eq 0 ] && { [ "$refs_named" -eq 0 ] || [ -n "$seg_refspecs" ]; }; then
    gates_commits=1
    refspecs=$refspecs$seg_refspecs
  fi
  # What a push with no refspec publishes depends on push.default.
  [ "$refs_named" -eq 0 ] && [ "$deleting" -eq 0 ] && bare_push=1
}

# `gh pr create` publishes the branch named by --head, which need not be the
# HEAD the diff describes.
parse_pr_create() {
  local w i=0 pending_value=0
  creates_pr=1
  gates_commits=1
  while [ "$i" -lt "${#seg_args[@]}" ]; do
    w=${seg_args[$i]}
    i=$((i + 1))
    if [ "$pending_value" -eq 1 ]; then
      pr_head=$w
      pending_value=0
      continue
    fi
    case "$w" in
      --head | -H) pending_value=1 ;;
      --head=*) pr_head=${w#--head=} ;;
    esac
  done
}

# Marks a command that moves HEAD or rewrites tracked files before the push:
# the gate runs now, on a tree that command is about to replace.
check_head_change() {
  case "$seg_sub" in
    switch) mark_unverifiable "a branch change runs first" ;;
    checkout)
      # `git checkout -- <path>` restores files the gate reads either way.
      local w i=0 restores_paths=0
      while [ "$i" -lt "${#seg_args[@]}" ]; do
        w=${seg_args[$i]}
        i=$((i + 1))
        [ "$w" = "--" ] && restores_paths=1
      done
      [ "$restores_paths" -eq 1 ] || mark_unverifiable "a branch change runs first"
      ;;
    merge | rebase | pull | cherry-pick | revert | reset | am | apply)
      mark_unverifiable "'git $seg_sub' runs first and replaces the tree the gate reads"
      ;;
    stash)
      case "${seg_args[0]:-}" in
        pop | apply)
          mark_unverifiable "'git stash ${seg_args[0]}' runs first and replaces the tree the gate reads"
          ;;
      esac
      ;;
  esac
}

input=$(cat) || fail "could not read stdin"

# Coarse prefilter on the raw JSON, so most commands exit without spawning jq.
# It matches loosely, because the exact spelling only becomes visible once the
# command below is normalized and split into words.
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

# Split the command into simple commands on `&&`, `||`, `;` and newlines, with
# `||` leaving a marker behind. Each piece of a pipeline gets a leading `|`
# marker, because a `cd` there runs in a subshell and does not move the push.
# The split ignores quoting, so a quoted separator splits too.
segments=$(printf '%s\n' "$norm_cmd" |
  awk -v or_mark="$OR_MARK" '{ gsub(/\|\|/, "\n" or_mark "\n"); gsub(/&&/, "\n"); gsub(/;/, "\n"); print }' |
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
cd_last=0
or_pending=0

while IFS= read -r seg; do
  # Apply the subshell exits of the previous segment: each `)` restores the
  # directory its `(` saved, after that segment's own `cd` has been read.
  while [ "$pending_closes" -gt 0 ] && [ -n "$dir_stack" ]; do
    run_dir=${dir_stack%%"$NL"*}
    dir_stack=${dir_stack#*"$NL"}
    pending_closes=$((pending_closes - 1))
  done
  pending_closes=0

  # Only one branch of an AND-OR list runs, so a `cd` whose success the shell
  # tests, and a `cd` that runs only when an earlier command fails, both leave
  # the push in a directory the hook cannot name.
  if [ "$seg" = "$OR_MARK" ]; then
    [ "$push_seen" -eq 0 ] && [ "$cd_last" -eq 1 ] &&
      mark_unverifiable "a conditional cd runs first"
    or_pending=1
    continue
  fi

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

  cd_this=0
  if scan_segment "$seg"; then
    if [ "$seg_cmd" = git ] && [ "$seg_sub" = push ]; then
      push_seen=1
      if [ "$seg_bypass" -eq 0 ]; then
        all_bypassed=0
        [ -n "$seg_override" ] && mark_unverifiable "$seg_override"
        parse_push_segment
      fi
    elif [ "$seg_cmd" = gh ] && [ "$seg_sub" = pr ] && [ "$seg_sub2" = create ]; then
      push_seen=1
      if [ "$seg_bypass" -eq 0 ]; then
        all_bypassed=0
        [ -n "$seg_override" ] && mark_unverifiable "$seg_override"
        parse_pr_create
      fi
    elif [ "$seg_cmd" = git ] && [ "$push_seen" -eq 0 ] && [ "$piped" -eq 0 ]; then
      check_head_change
    fi
  else
    case "$seg" in
      # A push the word scan cannot read, such as one inside `bash -c`, still
      # publishes commits, so the full gate runs.
      *"git push"* | *"gh pr create"*)
        push_seen=1
        if [ "$seg_bypass" -eq 0 ]; then
          all_bypassed=0
          gates_commits=1
        fi
        ;;
      cd | "cd "*)
        cd_this=1
        if [ "$push_seen" -eq 0 ] && [ "$or_pending" -eq 1 ]; then
          mark_unverifiable "a conditional cd runs first"
        fi
        maybe_cd "$seg"
        ;;
    esac
  fi
  cd_last=$cd_this
  or_pending=0
done <<SEGMENTS
$segments
SEGMENTS

# Nothing in the command publishes anything.
[ "$push_seen" -eq 0 ] && exit 0

# Every push segment carries the bypass.
[ "$all_bypassed" -eq 1 ] && exit 0

# Without the target of a `cd`, the hook cannot tell which tree the push comes
# from, and validating any other tree could pass a branch nobody tested. Exit 2
# here means "not verified", and a standalone `cd` call avoids it.
if [ -n "$unresolved_cd" ]; then
  printf "ci-before-push: can't resolve '%s'; run the cd as its own Bash call, then push (or prefix SKIP_CI_HOOK=1).\n" "$unresolved_cd" >&2
  exit 2
fi

# An inherited GIT_DIR or GIT_WORK_TREE aims git at a repository other than the
# one holding the tree the hook reads.
if [ -n "${GIT_DIR:-}" ] || [ -n "${GIT_WORK_TREE:-}" ]; then
  mark_unverifiable "GIT_DIR or GIT_WORK_TREE is set in the environment"
fi

[ -n "$unverifiable" ] && block_unverifiable "$unverifiable"

# Every push segment only deletes remote refs.
[ "$gates_commits" -eq 0 ] && exit 0

if ! repo_root=$(git -C "$run_dir" rev-parse --show-toplevel 2>/dev/null); then
  block_unverifiable "$run_dir is not inside a git repository"
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
  base_ref=origin/main
  if [ "$creates_pr" -eq 0 ] &&
    upstream=$(git -C "$repo_root" rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null) &&
    git -C "$repo_root" merge-base --is-ancestor "$upstream" HEAD 2>/dev/null; then
    base_ref=$upstream
  fi

  # The classifier prints `none` for an empty diff and `all` for a base it
  # cannot diff against. A failure, no output, or an unknown line selects
  # every gate.
  run_all=1
  scope_args=()
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
