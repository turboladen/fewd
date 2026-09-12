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
  { id: 'lint', label: 'eslint', tier: 'ci', cmd: ['bun', 'run', 'lint'] },
  { id: 'types', label: 'tsc --noEmit', tier: 'extra', cmd: ['bunx', 'tsc', '--noEmit'] },
  { id: 'lockfile', label: 'bun.lock is current', tier: 'ci', cmd: ['bun', 'install', '--frozen-lockfile'] },
  { id: 'fe-test', label: 'vitest', tier: 'ci', cmd: ['bun', 'run', 'test'] },
  { id: 'clippy', label: 'clippy', tier: 'ci', cwd: SERVER, cmd: ['cargo', 'clippy', '--all-targets', '--all-features', '--', '-D', 'warnings'] },
  { id: 'rust-test', label: 'cargo test', tier: 'ci', cwd: SERVER, cmd: ['cargo', 'test', '--all-features'] },
  { id: 'smoke', label: 'API + MCP smoke', tier: 'extra', cmd: ['bun', '.claude/skills/run-fewd-server/driver.mjs', 'smoke'] },
  { id: 'migration', label: 'migration drift', tier: 'ci', slow: true, cmd: ['bash', 'scripts/migration-smoke-test.sh'] },
];

const FIXERS = [
  { label: 'cargo fmt', cwd: SERVER, cmd: ['cargo', 'fmt', '--all'] },
  { label: 'dprint fmt', cmd: ['dprint', 'fmt'] },
  { label: 'eslint --fix', cmd: ['bun', 'run', 'lint:fix'] },
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
  const flag = (f) => argv.includes(f);
  const onlyAt = argv.indexOf('--only');
  const only = onlyAt === -1 ? null : argv[onlyAt + 1];

  // Reject anything unrecognized. A silently ignored flag runs every gate and
  // reports PASS, which reads as though the flag worked.
  const KNOWN = ['--fast', '--ci-only', '--fix', '--list', '--only'];
  const stray = argv.filter((a, i) => a.startsWith('-') && !KNOWN.includes(a) || (i !== onlyAt + 1 && !a.startsWith('-')));
  if (stray.length > 0) {
    console.error(`unrecognized argument: ${stray.join(' ')}\nusage: verify.mjs [--fast] [--ci-only] [--fix] [--only <id>] [--list]`);
    return 1;
  }
  if (onlyAt !== -1 && !only) {
    console.error('--only needs a gate id; see --list');
    return 1;
  }

  if (flag('--list')) {
    for (const g of GATES) console.log(`${g.id.padEnd(10)} ${g.tier.padEnd(6)} ${g.slow ? 'slow  ' : '      '} ${g.label}`);
    return 0;
  }

  const gates = selectGates({ fast: flag('--fast'), ciOnly: flag('--ci-only'), only });
  if (gates.length === 0) {
    console.error(`no gate matches --only ${only}; see --list`);
    return 1;
  }

  const note = ensureDist();
  if (note) console.log(`note: ${note}\n`);

  if (flag('--fix')) {
    for (const f of FIXERS) {
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
  console.log(`\n${results.length} gates in ${secs(total)}`);
  if (failed.length === 0) {
    console.log(flag('--fast') ? 'PASS (--fast: migration drift not run)' : 'PASS');
    return 0;
  }
  const blocking = failed.filter((f) => f.tier === 'ci');
  console.log(`FAIL: ${failed.map((f) => f.id).join(', ')}`);
  if (blocking.length === 0) console.log('(none of these block the merge, but they break the build or the running app)');
  return 1;
}

process.exit(await main());
