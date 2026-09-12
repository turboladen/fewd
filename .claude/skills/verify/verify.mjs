#!/usr/bin/env bun
// Runs every gate `.github/workflows/ci.yml` enforces, plus a few it does
// not, and reports all of them in one pass. Run from the repo root.
//
// The gate list below mirrors ci.yml step for step. When a step there
// changes, change it here — a green run has to mean "this branch will pass
// CI", and it stops meaning that the moment the two lists drift.

import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const SERVER = join(REPO, 'server');

// `ci` gates block the merge. `extra` gates do not — they cover the two ways
// this repo breaks after a green CI run: a type error that only `tsc` sees
// (CI never runs the build) and a runtime break in the API/MCP surface.
const GATES = [
  { id: 'fmt', label: 'cargo fmt', tier: 'ci', cwd: SERVER, cmd: ['cargo', 'fmt', '--all', '--', '--check'] },
  { id: 'dprint', label: 'dprint check', tier: 'ci', cmd: ['dprint', 'check'] },
  { id: 'typos', label: 'typos', tier: 'ci', cmd: ['typos', '--config', '.typos.toml'] },
  // `bun install --frozen-lockfile` installs, so it runs before every gate
  // that reads `node_modules` — otherwise eslint, tsc and vitest report
  // against a tree the later install then changes.
  { id: 'lockfile', label: 'bun.lock is current', tier: 'ci', cmd: ['bun', 'install', '--frozen-lockfile'] },
  { id: 'lint', label: 'eslint', tier: 'ci', cmd: ['bun', 'run', 'lint'] },
  { id: 'types', label: 'tsc --noEmit', tier: 'extra', cmd: ['bunx', 'tsc', '--noEmit'] },
  { id: 'fe-test', label: 'vitest', tier: 'ci', cmd: ['bun', 'run', 'test'] },
  { id: 'clippy', label: 'clippy', tier: 'ci', cwd: SERVER, cmd: ['cargo', 'clippy', '--all-targets', '--all-features', '--', '-D', 'warnings'] },
  { id: 'rust-test', label: 'cargo test', tier: 'ci', cwd: SERVER, cmd: ['cargo', 'test', '--all-features'] },
  { id: 'smoke', label: 'API + MCP smoke', tier: 'extra', cmd: ['bun', '.claude/skills/run-fewd-server/driver.mjs', 'smoke'] },
  { id: 'migration', label: 'migration drift', tier: 'ci', slow: true, cmd: ['bash', 'scripts/migration-smoke-test.sh'] },
];

// Each fixer names the gate it rewrites for, so `--fix --only <id>` runs the
// one fixer that gate needs instead of reformatting the whole repo.
const FIXERS = [
  { gate: 'fmt', label: 'cargo fmt', cwd: SERVER, cmd: ['cargo', 'fmt', '--all'] },
  { gate: 'dprint', label: 'dprint fmt', cmd: ['dprint', 'fmt'] },
  { gate: 'lint', label: 'eslint --fix', cmd: ['bun', 'run', 'lint:fix'] },
];

// Which gates a plain `verify` runs. Everything, ordered cheapest first, so a
// formatting slip surfaces in a second instead of after the release build.
// `--fast` drops the one gate that costs more than a few seconds.
function selectGates({ fast, ciOnly, only }) {
  let gates = GATES;
  if (only) gates = gates.filter((g) => g.id === only);
  if (ciOnly) gates = gates.filter((g) => g.tier === 'ci');
  if (fast) gates = gates.filter((g) => !g.slow);
  return gates;
}

function exec({ cmd, cwd }) {
  return new Promise((res) => {
    const started = Date.now();
    const p = spawn(cmd[0], cmd.slice(1), { cwd: cwd ?? REPO, stdio: ['ignore', 'pipe', 'pipe'] });
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
  mkdirSync(dist, { recursive: true });
  writeFileSync(join(dist, '.gitkeep'), '');
  return 'created a placeholder dist/ — run `bun run build` for a real bundle';
}

const secs = (ms) => `${(ms / 1000).toFixed(1)}s`;

async function main() {
  const argv = process.argv.slice(2);
  const USAGE = 'usage: verify.mjs [--fast] [--ci-only] [--fix] [--only <id>] [--list]';

  // Walk the arguments rather than scanning for each flag. Anything
  // unrecognized has to be an error: an argument that is silently dropped runs
  // every gate and reports PASS, which reads as though it worked.
  const opts = { fast: false, ciOnly: false, fix: false, list: false, only: null };
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
      case '--only': {
        const value = argv[++i];
        if (!value || value.startsWith('-')) {
          console.error('--only needs a gate id; see --list');
          return 1;
        }
        opts.only = value;
        break;
      }
      default:
        console.error(`unrecognized argument: ${argv[i]}\n${USAGE}`);
        return 1;
    }
  }

  if (opts.list) {
    for (const g of GATES) console.log(`${g.id.padEnd(10)} ${g.tier.padEnd(6)} ${g.slow ? 'slow  ' : '      '} ${g.label}`);
    return 0;
  }

  if (opts.only && !GATES.some((g) => g.id === opts.only)) {
    console.error(`no gate matches --only ${opts.only}; see --list`);
    return 1;
  }

  const gates = selectGates(opts);
  if (gates.length === 0) {
    console.error(`--only ${opts.only} is excluded by ${opts.ciOnly ? '--ci-only' : '--fast'}`);
    return 1;
  }

  const note = ensureDist();
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
    // Only a full run answers "will this branch pass CI", so a narrowed run
    // says how many gates it never looked at.
    const skipped = GATES.length - results.length;
    console.log(skipped === 0 ? 'PASS' : `PASS (${skipped} of ${GATES.length} gates not run)`);
    return 0;
  }
  const blocking = failed.filter((f) => f.tier === 'ci');
  console.log(`FAIL: ${failed.map((f) => f.id).join(', ')}`);
  if (blocking.length === 0) console.log('(none of these block the merge, but they break the build or the running app)');
  return 1;
}

process.exit(await main());
