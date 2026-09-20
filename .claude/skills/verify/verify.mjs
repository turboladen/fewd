#!/usr/bin/env bun
// Runs the gates `.github/workflows/ci.yml` enforces, plus a few it does not,
// and reports all of them in one pass. Run from the repo root.
//
// The gate list below mirrors every check ci.yml runs. When a check there
// changes, change it here — a green run has to mean "this branch will pass
// CI", and it stops meaning that the moment the two lists drift. Which gates
// a change selects lives in scripts/changed-scopes.mjs, shared with ci.yml
// and the pre-push hook.

import { spawn } from 'node:child_process';
import { existsSync, lstatSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { GATE_SCOPES, SCOPES, changedPaths, classifyPaths, describeScopes, gateApplies } from '../../../scripts/changed-scopes.mjs';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');

// `ci` gates block the merge. `extra` gates do not — they cover the two ways
// this repo breaks after a green CI run: a type error that only `tsc` sees
// (CI never runs the build) and a runtime break in the API/MCP surface.
const GATES = [
  { id: 'fmt', rust: true, label: 'cargo fmt', tier: 'ci', cmd: ['cargo', 'fmt', '--all', '--', '--check'] },
  { id: 'dprint', label: 'dprint check', tier: 'ci', cmd: ['dprint', 'check'] },
  { id: 'typos', label: 'typos', tier: 'ci', cmd: ['typos', '--config', '.typos.toml'] },
  // `bun install --frozen-lockfile` installs, so it runs before every gate
  // that reads `node_modules` — otherwise eslint, tsc and vitest report
  // against a tree the later install then changes.
  { id: 'lockfile', label: 'bun.lock is current', tier: 'ci', cmd: ['bun', 'install', '--frozen-lockfile'] },
  { id: 'lint', label: 'eslint', tier: 'ci', cmd: ['bun', 'run', 'lint'] },
  { id: 'types', label: 'tsc --noEmit', tier: 'extra', cmd: ['bunx', 'tsc', '--noEmit'] },
  { id: 'fe-test', label: 'vitest', tier: 'ci', cmd: ['bun', 'run', 'test'] },
  // Clippy and tests run once per package, as ci.yml does. The comment on
  // those steps in ci.yml explains why.
  { id: 'clippy', rust: true, label: 'clippy (fewd-server)', tier: 'ci', cmd: ['cargo', 'clippy', '-p', 'fewd-server', '--all-targets', '--all-features', '--', '-D', 'warnings'] },
  { id: 'mig-clippy', rust: true, label: 'clippy (migration)', tier: 'ci', cmd: ['cargo', 'clippy', '-p', 'migration', '--all-targets', '--', '-D', 'warnings'] },
  { id: 'rust-test', rust: true, label: 'cargo test (fewd-server)', tier: 'ci', cmd: ['cargo', 'test', '-p', 'fewd-server', '--all-features'] },
  { id: 'mig-test', rust: true, label: 'cargo test (migration)', tier: 'ci', cmd: ['cargo', 'test', '-p', 'migration'] },
  { id: 'smoke', rust: true, label: 'API + MCP smoke', tier: 'extra', cmd: ['bun', '.claude/skills/run-fewd-server/driver.mjs', 'smoke'] },
  { id: 'migration', rust: true, label: 'migration drift', tier: 'ci', slow: true, cmd: ['bash', 'scripts/migration-smoke-test.sh'] },
];

// Each fixer names the gate it rewrites for, so `--fix --only <id>` runs the
// one fixer that gate needs instead of reformatting the whole repo.
const FIXERS = [
  { gate: 'fmt', label: 'cargo fmt', cmd: ['cargo', 'fmt', '--all'] },
  { gate: 'dprint', label: 'dprint fmt', cmd: ['dprint', 'fmt'] },
  { gate: 'lint', label: 'eslint --fix', cmd: ['bun', 'run', 'lint:fix'] },
];

// Gates stay in cheapest-first order, so a formatting slip surfaces in a
// second instead of after the release build. `scopes` is null for `--only`
// and `--all`, which ignore the changed paths.
function selectGates({ fast, ciOnly, only, scopes, skip }) {
  let gates = GATES;
  if (only) gates = gates.filter((g) => g.id === only);
  if (scopes) gates = gates.filter((g) => gateApplies(g.id, scopes));
  if (ciOnly) gates = gates.filter((g) => g.tier === 'ci');
  if (fast) gates = gates.filter((g) => !g.slow);
  if (skip.length) gates = gates.filter((g) => !skip.includes(g.id));
  return gates;
}

function exec({ cmd }) {
  return new Promise((res) => {
    const started = Date.now();
    const p = spawn(cmd[0], cmd.slice(1), { cwd: REPO, stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '';
    p.stdout.on('data', (d) => (out += d));
    p.stderr.on('data', (d) => (out += d));
    // A missing binary emits 'error' and never emits 'close'.
    p.on('error', (err) => res({ code: 127, out: `${cmd[0]}: ${err.message}`, ms: Date.now() - started }));
    p.on('close', (code) => res({ code: code ?? 1, out, ms: Date.now() - started }));
  });
}

// RustEmbed's `#[folder = "../dist"]` is checked when the macro expands, and a
// missing folder reports as three bogus `no associated function named 'get'`
// errors under one real one. CI creates a placeholder before its Rust steps;
// so does this, for the same reason.
function ensureDist() {
  const dist = join(REPO, 'dist');
  if (existsSync(dist)) return null;
  // existsSync follows symlinks, so a dangling dist symlink reads as missing,
  // and mkdirSync throws EEXIST on it.
  if (lstatSync(dist, { throwIfNoEntry: false })) {
    return 'dist/ is a broken symlink; point it at a built bundle, or the Rust gates cannot compile';
  }
  mkdirSync(dist, { recursive: true });
  writeFileSync(join(dist, '.gitkeep'), '');
  return 'created a placeholder dist/ — run `bun run build` for a real bundle';
}

const secs = (ms) => `${(ms / 1000).toFixed(1)}s`;

async function main() {
  const argv = process.argv.slice(2);
  const USAGE =
    'usage: verify.mjs [--fast] [--ci-only] [--fix] [--list]\n' +
    '                  [--only <id> | --all | --scope <scope>...] [--base <rev>] [--skip <id>...]';

  for (const id of Object.keys(GATE_SCOPES)) {
    if (!GATES.some((g) => g.id === id)) console.error(`warning: GATE_SCOPES names ${id}, which is not a gate`);
  }

  // Walk the arguments rather than scanning for each flag. Anything
  // unrecognized has to be an error: an argument that is silently dropped runs
  // every gate and reports PASS, which reads as though it worked.
  const opts = { fast: false, ciOnly: false, fix: false, list: false, all: false, only: null, scopes: [], skip: [], base: null };
  const valueOf = (flag, i, what) => {
    const value = argv[i];
    if (!value || value.startsWith('-')) throw new Error(`${flag} needs ${what}; see --list`);
    return value;
  };
  try {
    for (let i = 0; i < argv.length; i++) {
      switch (argv[i]) {
        case '--fast':
          opts.fast = true;
          break;
        case '--ci-only':
          opts.ciOnly = true;
          break;
        case '--fix':
          opts.fix = true;
          break;
        case '--list':
          opts.list = true;
          break;
        case '--all':
          opts.all = true;
          break;
        case '--only':
          opts.only = valueOf('--only', ++i, 'a gate id');
          break;
        case '--scope': {
          const scope = valueOf('--scope', ++i, 'a scope');
          if (![...SCOPES, 'all'].includes(scope)) throw new Error(`unknown scope ${scope}; scopes are ${[...SCOPES, 'all'].join(', ')}`);
          opts.scopes.push(scope);
          break;
        }
        case '--skip': {
          const id = valueOf('--skip', ++i, 'a gate id');
          if (!GATES.some((g) => g.id === id)) throw new Error(`no gate matches --skip ${id}; see --list`);
          opts.skip.push(id);
          break;
        }
        case '--base':
          opts.base = valueOf('--base', ++i, 'a git revision');
          break;
        default:
          throw new Error(`unrecognized argument: ${argv[i]}\n${USAGE}`);
      }
    }
  } catch (err) {
    console.error(err.message);
    return 1;
  }

  if (opts.list) {
    for (const g of GATES) {
      const scopes = (GATE_SCOPES[g.id] ?? ['any change']).join(',');
      console.log(`${g.id.padEnd(10)} ${g.tier.padEnd(6)} ${g.slow ? 'slow  ' : '      '} ${scopes.padEnd(19)} ${g.label}`);
    }
    return 0;
  }

  if ([opts.only, opts.all, opts.scopes.length > 0].filter(Boolean).length > 1) {
    console.error('--only, --all and --scope each choose the gates; pass one of them');
    return 1;
  }

  if (opts.only && !GATES.some((g) => g.id === opts.only)) {
    console.error(`no gate matches --only ${opts.only}; see --list`);
    return 1;
  }

  if (opts.base && (opts.only || opts.all || opts.scopes.length)) {
    console.error('--base only applies when the changed paths choose the gates; drop --only, --all or --scope');
    return 1;
  }

  // Without --only or --all, the changed paths choose the gates. A change list
  // that cannot be computed selects every gate rather than guessing.
  let scopes = null;
  if (opts.scopes.length) {
    scopes = opts.scopes;
    console.log(`scopes: ${scopes.join(', ')} (from --scope)\n`);
  } else if (!opts.only && !opts.all) {
    const base = opts.base ?? 'origin/main';
    const paths = changedPaths({ base, cwd: REPO });
    if (paths === null) {
      scopes = ['all'];
      console.log(`note: could not list changes against ${base}; running every gate\n`);
    } else if (paths.length === 0) {
      console.log(`no changes vs ${base}; use --all to run every gate`);
      return 0;
    } else {
      const result = classifyPaths(paths);
      scopes = result.scopes;
      console.log(`scopes vs ${base}:\n${describeScopes(result).join('\n')}\n`);
    }
  }

  const gates = selectGates({ ...opts, scopes });
  if (gates.length === 0) {
    console.error('no gates left to run after --only, --ci-only, --fast and --skip');
    return 1;
  }

  // Only the Rust gates, marked `rust: true`, touch the crate that embeds dist/.
  const note = gates.some((g) => g.rust) ? ensureDist() : null;
  if (note) console.log(`note: ${note}\n`);

  if (opts.fix) {
    const selected = new Set(gates.map((g) => g.id));
    for (const f of FIXERS.filter((f) => selected.has(f.gate))) {
      const r = await exec(f);
      // A formatter exits non-zero when something is left that it cannot
      // rewrite on its own — the gate run below reports what.
      console.log(`  fix ${f.label} ${r.code === 0 ? 'ok' : 'left issues it cannot fix'}`);
      if (r.code !== 0) console.log(r.out.trimEnd().split('\n').slice(-10).join('\n'));
    }
    console.log('');
  }

  const results = [];
  for (const g of gates) {
    process.stdout.write(`▸ ${g.label} … `);
    const r = await exec(g);
    results.push({ ...g, ...r });
    console.log(r.code === 0 ? `ok (${secs(r.ms)})` : `FAILED exit ${r.code} (${secs(r.ms)})`);
  }

  const failed = results.filter((r) => r.code !== 0);
  for (const f of failed) {
    console.log(`\n─── ${f.label} (${f.tier}) ───`);
    console.log(f.out.trimEnd().split('\n').slice(-25).join('\n'));
  }

  const total = results.reduce((a, r) => a + r.ms, 0);
  console.log(`\n${results.length} ${results.length === 1 ? 'gate' : 'gates'} in ${secs(total)}`);
  if (failed.length === 0) {
    // A narrowed run says how many gates it never looked at. A run narrowed
    // only by scope still answers "will CI pass", because CI selects its jobs
    // from the same scopes.
    const skipped = GATES.length - results.length;
    const why = scopes && !scopes.includes('all') ? `; scopes: ${scopes.join(', ')}` : '';
    console.log(skipped === 0 ? 'PASS' : `PASS (${skipped} of ${GATES.length} gates not run${why})`);
    return 0;
  }
  const blocking = failed.filter((f) => f.tier === 'ci');
  console.log(`FAIL: ${failed.map((f) => f.id).join(', ')}`);
  if (blocking.length === 0) console.log('(none of these block the merge, but they break the build or the running app)');
  return 1;
}

process.exit(await main());
