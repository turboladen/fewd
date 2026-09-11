#!/usr/bin/env bun
// Drives a disposable fewd-server: HTTP API over fetch, MCP over
// Streamable HTTP (bearer token + JSON-RPC handshake + SSE framing).
//
// Every instance it boots gets its own SQLite file under target/fewd-driver/
// and an ephemeral port, so nothing here can touch the family's data/fewd.db
// or collide with `just dev` on :3000. Run from the repo root.

import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, openSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const RUNDIR = join(REPO, 'target', 'fewd-driver');
const STATE = join(RUNDIR, 'state.json');
const LOG = join(RUNDIR, 'server.log');

const MCP_ACCEPT = 'application/json, text/event-stream';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function readState() {
  if (!existsSync(STATE)) die('no running instance — start one with `driver.mjs up` (or use `smoke`)');
  return JSON.parse(readFileSync(STATE, 'utf8'));
}

// Abort the current command. This throws rather than exiting so that a
// `finally` block still runs — `smoke` tears its server down there, and
// `process.exit` would skip it and leak the child. The top-level catch
// prints the message and exits 1.
function die(msg) {
  throw new Error(msg);
}

function run(cmd, args, opts = {}) {
  return new Promise((res, rej) => {
    const p = spawn(cmd, args, { cwd: REPO, stdio: 'inherit', ...opts });
    // A spawn failure (missing binary) emits 'error' and may never emit
    // 'exit', which would hang the driver instead of reporting it.
    p.on('error', rej);
    p.on('exit', (code) => (code === 0 ? res() : rej(new Error(`${cmd} exited ${code}`))));
  });
}

// ─── lifecycle ──────────────────────────────────────────────────

async function boot({ fresh = true } = {}) {
  mkdirSync(RUNDIR, { recursive: true });
  // A leftover instance holds the SQLite file and makes the next boot
  // exit 101 on a locked database.
  if (existsSync(STATE)) down();
  const db = join(RUNDIR, 'fewd.db');
  if (fresh) for (const s of ['', '-wal', '-shm']) rmSync(db + s, { force: true });

  console.error('driver: cargo build --bin fewd-server');
  await run('cargo', ['build', '--bin', 'fewd-server']);

  const bin = join(REPO, 'target', 'debug', 'fewd-server');
  // The child logs to a file descriptor rather than a Bun pipe: the port
  // line has to be readable while the process keeps running, and `up`
  // leaves it running after this script exits.
  const fd = openSync(LOG, 'w');
  const proc = Bun.spawn([bin], {
    cwd: REPO,
    env: {
      ...process.env,
      DATABASE_PATH: db,
      // PORT=0 asks the kernel for a free port; main.rs logs the bound one.
      PORT: '0',
      // The bound port is read back out of the log, so the directive that
      // carries that line is pinned last (most specific wins) — a surrounding
      // `RUST_LOG=warn` would otherwise silence it and every boot would fail
      // with "never announced a port".
      RUST_LOG: `${process.env.RUST_LOG ?? 'info'},fewd_server=info`,
      // An inherited MCP_ALLOWED_HOSTS (the dietpi deploy exports one) would
      // put the smoke test's off-allowlist Host on the allowlist and turn its
      // 403 assertion into a spurious failure.
      MCP_ALLOWED_HOSTS: '',
    },
    stdout: fd,
    stderr: fd,
  });

  // Record the pid before waiting on the port so a boot that never comes
  // up is still killable with `down` instead of leaking a child that holds
  // the scratch DB.
  writeFileSync(STATE, JSON.stringify({ pid: proc.pid, db, base: null, token: null, session: null }, null, 2));

  let port = null;
  for (let i = 0; i < 200 && port === null; i++) {
    await sleep(50);
    const m = readFileSync(LOG, 'utf8').match(/Server running on http:\/\/localhost:(\d+)/);
    if (m) port = Number(m[1]);
    else if (proc.exitCode !== null) {
      rmSync(STATE, { force: true });
      die(`server exited ${proc.exitCode}:\n${readFileSync(LOG, 'utf8').slice(-800)}`);
    }
  }
  if (port === null) {
    proc.kill();
    rmSync(STATE, { force: true });
    die(`server never announced a port in 10s:\n${readFileSync(LOG, 'utf8').slice(-800) || '(empty log)'}`);
  }

  const base = `http://localhost:${port}`;
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(`${base}/api/version`)).ok) break;
    } catch {}
    await sleep(50);
  }

  proc.unref();
  const state = { base, port, pid: proc.pid, db, token: null, session: null };
  writeFileSync(STATE, JSON.stringify(state, null, 2));
  return state;
}

function down() {
  if (!existsSync(STATE)) return console.error('driver: nothing running');
  const { pid } = JSON.parse(readFileSync(STATE, 'utf8'));
  try {
    process.kill(pid, 'SIGTERM');
    console.error(`driver: stopped pid ${pid}`);
  } catch {
    console.error(`driver: pid ${pid} already gone`);
  }
  rmSync(STATE, { force: true });
}

// ─── HTTP API ───────────────────────────────────────────────────

async function api(state, method, path, body) {
  const res = await fetch(`${state.base}${path.startsWith('/') ? path : '/' + path}`, {
    method,
    headers: body === undefined ? {} : { 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await res.text();
  if (!res.ok) throw new Error(`${method} ${path} → ${res.status}: ${text.slice(0, 300)}`);
  return text ? JSON.parse(text) : null;
}

// ─── MCP ────────────────────────────────────────────────────────

// Pull the JSON-RPC payload out of a Streamable HTTP response. The
// transport answers POSTs with an SSE stream, so the body arrives as
// `data: {...}` lines interleaved with keep-alive `data:`/`id:`/`retry:`
// frames — JSON.parse on the raw body always fails.
function parseSse(text) {
  for (const line of text.split('\n')) {
    if (line.startsWith('data: ') && line[6] === '{') return JSON.parse(line.slice(6));
  }
  throw new Error(`no JSON frame in MCP response: ${text.slice(0, 300)}`);
}

async function ensureToken(state) {
  if (state.token) return state;
  const people = await api(state, 'GET', '/api/people');
  const person = people.find((p) => p.is_active) ?? die('no active people to issue a token to');
  const { token } = await api(state, 'POST', `/api/people/${person.id}/mcp-token`, {});
  state.token = token;
  state.person = { id: person.id, name: person.name };
  writeFileSync(STATE, JSON.stringify(state, null, 2));
  return state;
}

async function mcpPost(state, payload, extraHeaders = {}) {
  const headers = {
    Authorization: `Bearer ${state.token}`,
    'Content-Type': 'application/json',
    Accept: MCP_ACCEPT,
    ...extraHeaders,
  };
  if (state.session) headers['mcp-session-id'] = state.session;
  const res = await fetch(`${state.base}/mcp`, { method: 'POST', headers, body: JSON.stringify(payload) });
  return { res, text: await res.text() };
}

// initialize → capture `mcp-session-id` → notifications/initialized.
// rmcp's `initialize` is served by `get_info(&self)` and never reads the
// request extensions, so the auth identity only reaches tools after the
// notification lands; skipping it makes every tools/call fail.
async function handshake(state) {
  if (state.session) return state;
  const { res, text } = await mcpPost(state, {
    jsonrpc: '2.0',
    id: 1,
    method: 'initialize',
    params: {
      protocolVersion: '2025-06-18',
      capabilities: {},
      clientInfo: { name: 'fewd-driver', version: '0' },
    },
  });
  if (!res.ok) throw new Error(`initialize → ${res.status}: ${text.slice(0, 300)}`);
  parseSse(text);
  state.session = res.headers.get('mcp-session-id') ?? die('initialize returned no mcp-session-id');
  const ack = await mcpPost(state, { jsonrpc: '2.0', method: 'notifications/initialized' });
  if (ack.res.status !== 202) throw new Error(`notifications/initialized → ${ack.res.status}`);
  writeFileSync(STATE, JSON.stringify(state, null, 2));
  return state;
}

let rpcId = 100;
async function mcp(state, method, params) {
  await ensureToken(state);
  await handshake(state);
  let { res, text } = await mcpPost(state, { jsonrpc: '2.0', id: ++rpcId, method, ...(params ? { params } : {}) });
  // A 404 means the server has forgotten this session id — the recorded one
  // outlived a restart. Re-handshake once rather than making the caller
  // clear the state file by hand.
  if (res.status === 404 && state.session) {
    state.session = null;
    await handshake(state);
    ({ res, text } = await mcpPost(state, { jsonrpc: '2.0', id: ++rpcId, method, ...(params ? { params } : {}) }));
  }
  if (!res.ok) throw new Error(`${method} → ${res.status}: ${text.slice(0, 300)}`);
  const body = parseSse(text);
  if (body.error) throw new Error(`${method} → JSON-RPC error ${JSON.stringify(body.error)}`);
  return body.result;
}

// Tool results arrive as `content: [{type:"text", text}]`; most fewd tools
// put JSON in that text, so unwrap one layer when it parses.
function toolText(result) {
  const text = (result.content ?? []).map((c) => c.text ?? '').join('\n');
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

// ─── smoke ──────────────────────────────────────────────────────

const ok = (label, detail) => console.log(`  ✓ ${label}${detail ? ` — ${detail}` : ''}`);

async function smoke() {
  console.log('▸ boot');
  const state = await boot({ fresh: true });
  ok('server up', state.base);

  try {
    console.log('▸ HTTP API');
    const version = await api(state, 'GET', '/api/version');
    ok('GET /api/version', `${version.version} @ ${version.git_sha}`);

    // A fresh DB is seeded with four sample people by seed_data::seed_if_empty.
    const seeded = await api(state, 'GET', '/api/people');
    ok('GET /api/people', `${seeded.length} seeded`);

    const person = await api(state, 'POST', '/api/people', {
      name: 'Driver Smoke',
      birthdate: '1990-01-01',
      dislikes: [],
      favorites: [],
    });
    ok('POST /api/people', person.name);

    const recipe = await api(state, 'POST', '/api/recipes', {
      name: 'Smoke Test Chili',
      source: 'manual',
      servings: 4,
      instructions: '1. Brown the beef.\n2. Add beans and simmer 30 minutes.',
      ingredients: [
        { name: 'ground beef', amount: { type: 'single', value: 1 }, unit: 'lb', notes: null },
        { name: 'kidney beans', prep: 'drained', amount: { type: 'single', value: 2 }, unit: 'can', notes: null },
      ],
      tags: ['dinner', 'one-pot'],
    });
    ok('POST /api/recipes', `${recipe.name} (${recipe.slug})`);

    // meal_type must be Title Case and order_index is the planner slot
    // (Breakfast=0 Lunch=1 Dinner=2 Snack=3), not a sort key.
    const meal = await api(state, 'POST', '/api/meals', {
      date: '2026-09-14',
      meal_type: 'Dinner',
      order_index: 2,
      servings: [
        { food_type: 'recipe', person_id: person.id, recipe_id: recipe.id, servings_count: 1.0, notes: null },
      ],
    });
    ok('POST /api/meals', `${meal.meal_type} ${meal.date}`);

    const list = await api(state, 'GET', '/api/shopping-list?start_date=2026-09-14&end_date=2026-09-14');
    const beef = list.find((i) => i.ingredient_name === 'ground beef');
    if (!beef) throw new Error('shopping list missing the planned recipe');
    // total_amount is optional on the wire (an unparsed amount aggregates to
    // null), so read it defensively rather than dereferencing into a TypeError.
    if (!beef.total_amount) throw new Error('shopping list line for beef carries no aggregated amount');
    ok('GET /api/shopping-list', `${list.length} lines, beef scaled to ${beef.total_amount.value} ${beef.total_unit}`);

    console.log('▸ MCP');
    const unauth = await fetch(`${state.base}/mcp`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Accept: MCP_ACCEPT },
      body: '{"jsonrpc":"2.0","id":1,"method":"tools/list"}',
    });
    if (unauth.status !== 401) throw new Error(`unauthenticated /mcp returned ${unauth.status}, expected 401`);
    ok('unauthenticated /mcp rejected', '401');

    await ensureToken(state);
    ok('token provisioned', `for ${state.person.name}`);
    await handshake(state);
    ok('initialize + initialized', state.session);

    const tools = (await mcp(state, 'tools/list')).tools;
    ok('tools/list', `${tools.length}: ${tools.map((t) => t.name).sort().join(', ')}`);

    const who = toolText(await mcp(state, 'tools/call', { name: 'whoami', arguments: {} }));
    if (!String(who).includes(state.person.name)) throw new Error(`whoami did not identify the caller: ${who}`);
    ok('tools/call whoami', String(who));

    const found = toolText(await mcp(state, 'tools/call', { name: 'search_recipes', arguments: { query: 'chili' } }));
    if (!found.some?.((r) => r.slug === recipe.slug)) throw new Error('search_recipes did not find the recipe just created');
    ok('tools/call search_recipes', `found ${recipe.slug} over the HTTP-created row`);

    const prompts = (await mcp(state, 'prompts/list')).prompts;
    const resources = (await mcp(state, 'resources/list')).resources;
    ok('prompts/list + resources/list', `${prompts.length} prompts, ${resources.length} resources`);

    const rebind = await fetch(`${state.base}/mcp`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${state.token}`, Host: 'dietpi.local', 'Content-Type': 'application/json', Accept: MCP_ACCEPT },
      body: '{"jsonrpc":"2.0","id":1,"method":"tools/list"}',
    });
    if (rebind.status !== 403) throw new Error(`off-allowlist Host returned ${rebind.status}, expected 403`);
    ok('off-allowlist Host rejected', '403 (MCP_ALLOWED_HOSTS opts LAN names in)');

    console.log('\nSMOKE OK');
  } finally {
    down();
  }
}

// ─── cli ────────────────────────────────────────────────────────

const [cmd, ...rest] = process.argv.slice(2);
const print = (v) => console.log(typeof v === 'string' ? v : JSON.stringify(v, null, 2));

try {
  switch (cmd) {
    case 'smoke':
      await smoke();
      break;
    case 'up': {
      const state = await boot({ fresh: !rest.includes('--keep-db') });
      await ensureToken(state);
      console.log(`base   ${state.base}`);
      console.log(`db     ${state.db}`);
      console.log(`log    ${LOG}`);
      console.log(`token  ${state.token}   (${state.person.name})`);
      break;
    }
    case 'down':
      down();
      break;
    case 'api':
      if (!rest[0] || !rest[1]) die('usage: api <METHOD> <path> [json]');
      print(await api(readState(), rest[0].toUpperCase(), rest[1], rest[2] ? JSON.parse(rest[2]) : undefined));
      break;
    case 'mcp':
      if (!rest[0]) die('usage: mcp <method> [params-json]');
      print(await mcp(readState(), rest[0], rest[1] ? JSON.parse(rest[1]) : undefined));
      break;
    case 'tool':
      if (!rest[0]) die('usage: tool <name> [args-json]');
      print(toolText(await mcp(readState(), 'tools/call', { name: rest[0], arguments: rest[1] ? JSON.parse(rest[1]) : {} })));
      break;
    default:
      console.error(`usage: bun .claude/skills/run-fewd-server/driver.mjs <command>

  smoke                       boot a throwaway instance, exercise API + MCP, tear down
  up [--keep-db]              boot one and leave it running; prints base URL and token
  down                        stop it
  api <METHOD> <path> [json]  one HTTP call against the running instance
  mcp <method> [params-json]  one MCP JSON-RPC call (handshakes on first use)
  tool <name> [args-json]     shorthand for tools/call`);
      process.exit(cmd ? 1 : 0);
  }
} catch (err) {
  console.error(`\ndriver: ${err.message}`);
  process.exit(1);
}
