#!/usr/bin/env node
// Maps changed paths to gate scopes. The verify skill, the pre-push hook and
// ci.yml all select their gates from this one definition.
//
// Usage: node scripts/changed-scopes.mjs [--base <rev>] [--no-worktree]
//                                        [--format text|lines|github] [--stdin]
//
// `--format lines` prints one scope per line, or `none` when nothing changed.
// Exit 0 whenever a scope set was printed, including the `all` fallback when
// the changed files cannot be listed. Exit 2 on a usage error, and any crash
// exits non-zero; callers treat a non-zero exit as `all`.

import { execFileSync } from 'node:child_process';
import { readFileSync, realpathSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

export const SCOPES = ['docs', 'frontend', 'rust'];

const FRONTEND_FILES = new Set([
  'index.html',
  'package.json',
  'bun.lock',
  'vite.config.ts',
  'vitest.config.ts',
  'eslint.config.js',
]);

// A file takes every scope whose rule matches, and a file no rule matches
// selects every gate. A non-markdown file under docs/ matches nothing on
// purpose: dprint formats JSON there and eslint lints JavaScript there. For the
// same reason, JavaScript and TypeScript under server/ match no rule either.
const SCRIPT_FILE = /\.[cm]?[jt]sx?$/;

const RULES = [
  { scope: 'docs', test: (p) => p.endsWith('.md') },
  { scope: 'docs', test: (p) => p.startsWith('LICENSE') || p === '.git-blame-ignore-revs' || p.startsWith('.beads/') },
  // dprint formats JSON. CI runs dprint only in Frontend Checks, which the
  // rust scope does not start, so JSON under server/ takes the docs scope too.
  { scope: 'docs', test: (p) => p.startsWith('server/') && p.endsWith('.json') },
  {
    scope: 'frontend',
    test: (p) =>
      p.startsWith('src/') || p.startsWith('public/') || FRONTEND_FILES.has(p) || /^tsconfig[^/]*\.json$/.test(p),
  },
  {
    scope: 'rust',
    test: (p) => (p.startsWith('server/') && !SCRIPT_FILE.test(p)) || p === 'Cargo.toml' || p === 'Cargo.lock',
  },
];

// Which scopes select each verify gate. `always` runs on any change. A gate
// missing from this table runs whenever anything changed. ci.yml mirrors this
// table in its job and step `if:` conditions.
export const GATE_SCOPES = {
  typos: ['always'],
  dprint: ['docs', 'frontend', 'rust'],
  lockfile: ['frontend'],
  lint: ['frontend'],
  types: ['frontend'],
  'fe-test': ['frontend'],
  fmt: ['rust'],
  clippy: ['rust'],
  'mig-clippy': ['rust'],
  'rust-test': ['rust'],
  'mig-test': ['rust'],
  smoke: ['rust'],
  migration: ['rust'],
};

/**
 * Classifies repo-relative paths. Returns the matched scopes in `SCOPES`
 * order, with `all` last when some path matched no rule, and the paths that
 * triggered each scope.
 */
export function classifyPaths(paths) {
  const triggers = {};
  const add = (scope, path) => (triggers[scope] ??= []).push(path);
  for (const raw of paths) {
    const path = raw.replace(/^\.\//, '');
    if (!path) continue;
    const matched = new Set(RULES.filter((r) => r.test(path)).map((r) => r.scope));
    if (matched.size === 0) add('all', path);
    for (const scope of matched) add(scope, path);
  }
  const scopes = [...SCOPES, 'all'].filter((s) => triggers[s]);
  return { scopes, triggers };
}

/** Reports whether a verify gate runs for a non-empty scope set. */
export function gateApplies(gateId, scopes) {
  if (scopes.includes('all')) return true;
  const selectors = GATE_SCOPES[gateId];
  if (!selectors || selectors.includes('always')) return true;
  return selectors.some((s) => scopes.includes(s));
}

function git(args, cwd) {
  return execFileSync('git', args, {
    cwd,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    maxBuffer: 64 * 1024 * 1024,
  });
}

const splitNul = (s) => s.split('\0').filter(Boolean);

/**
 * Lists the paths changed since the merge base of HEAD and `base`, plus staged,
 * unstaged and untracked paths unless `worktree` is false. Paths are relative
 * to the repository root. Returns null when the list cannot be computed, such
 * as an unknown base or a directory outside a repository.
 */
export function changedPaths({ base = 'origin/main', worktree = true, cwd = process.cwd() } = {}) {
  try {
    const root = git(['rev-parse', '--show-toplevel'], cwd).trim();
    const mergeBase = git(['merge-base', 'HEAD', base], root).trim();
    // `--no-relative` and running from the root keep every path root-relative,
    // whatever `diff.relative` says and wherever the caller stands.
    const diff = ['diff', '-z', '--name-only', '--no-renames', '--no-relative'];
    const paths = new Set(splitNul(git([...diff, mergeBase, 'HEAD'], root)));
    if (worktree) {
      for (const p of splitNul(git([...diff, 'HEAD'], root))) paths.add(p);
      for (const p of splitNul(git(['ls-files', '-z', '--others', '--exclude-standard'], root))) paths.add(p);
    }
    return [...paths].sort();
  } catch {
    return null;
  }
}

/** Formats a classification as indented lines naming each scope's first few triggering paths. */
export function describeScopes({ triggers, scopes }, limit = 3) {
  return scopes.map((scope) => {
    const paths = triggers[scope] ?? [];
    const more = paths.length > limit ? ` (+${paths.length - limit} more)` : '';
    return `  ${scope.padEnd(9)} ${paths.slice(0, limit).join(', ')}${more}`;
  });
}

/** Formats scope booleans as `$GITHUB_OUTPUT` lines. `all` sets every scope true. */
export function formatGithub(scopes) {
  const all = scopes.includes('all');
  return [...SCOPES.map((s) => `${s}=${all || scopes.includes(s)}`), `all=${all}`].join('\n') + '\n';
}

function parseArgs(argv) {
  const opts = { base: 'origin/main', worktree: true, format: 'text', stdin: false };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--no-worktree') opts.worktree = false;
    else if (arg === '--stdin') opts.stdin = true;
    else if (arg === '--base' || arg === '--format') {
      const value = argv[++i];
      if (!value || value.startsWith('-')) throw new Error(`${arg} needs a value`);
      opts[arg.slice(2)] = value;
    } else throw new Error(`unrecognized argument: ${arg}`);
  }
  if (!['text', 'lines', 'github'].includes(opts.format)) throw new Error(`unknown --format ${opts.format}`);
  return opts;
}

function main(argv) {
  let opts;
  try {
    opts = parseArgs(argv);
  } catch (err) {
    console.error(`changed-scopes: ${err.message}`);
    return 2;
  }

  let result;
  if (opts.stdin) {
    result = classifyPaths(readFileSync(0, 'utf8').split(/[\0\n]/));
  } else {
    const paths = changedPaths(opts);
    if (paths === null) {
      console.error(`changed-scopes: could not list changes against ${opts.base}; selecting every gate`);
      result = { scopes: ['all'], triggers: { all: [`(changes against ${opts.base} unknown)`] } };
    } else {
      result = classifyPaths(paths);
    }
  }

  const summary = result.scopes.length === 0 ? ['  (no changed files)'] : describeScopes(result);
  if (opts.format === 'lines') {
    // An explicit `none` lets a caller tell an empty diff from missing output.
    console.log(result.scopes.length === 0 ? 'none' : result.scopes.join('\n'));
  } else if (opts.format === 'github') {
    process.stdout.write(formatGithub(result.scopes));
    console.error(summary.join('\n'));
  } else {
    console.log(summary.join('\n'));
  }
  return 0;
}

// Compare real paths so a symlinked checkout still counts as a direct run, and
// importing this module from verify.mjs or a test never runs the CLI.
function invokedDirectly() {
  try {
    return !!process.argv[1] && realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url));
  } catch {
    return false;
  }
}

if (invokedDirectly()) process.exitCode = main(process.argv.slice(2));
