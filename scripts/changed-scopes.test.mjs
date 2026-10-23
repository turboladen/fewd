// @vitest-environment node
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it } from 'vitest';
import { GATES } from '../.claude/skills/verify/verify.mjs';
import { GATE_SCOPES, SCOPES, changedPaths, classifyPaths, formatGithub, gateApplies } from './changed-scopes.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const CLI = join(HERE, 'changed-scopes.mjs');

const scopesOf = (...paths) => classifyPaths(paths).scopes;

describe('classifyPaths', () => {
  it.each([
    ['README.md', ['docs']],
    ['.github/copilot-instructions.md', ['docs']],
    ['docs/mcp-testing.md', ['docs']],
    ['.claude/rules/ci.md', ['docs']],
    ['LICENSE', ['docs']],
    ['.beads/config.yaml', ['docs']],
    ['src/components/App.tsx', ['frontend']],
    ['public/favicon.svg', ['frontend']],
    ['index.html', ['frontend']],
    ['package.json', ['frontend']],
    ['bun.lock', ['frontend']],
    ['vite.config.ts', ['frontend']],
    ['tsconfig.node.json', ['frontend']],
    ['eslint.config.js', ['frontend']],
    ['server/src/main.rs', ['rust']],
    ['server/migration/src/lib.rs', ['rust']],
    ['server/src/entities/recipe.rs', ['rust']],
    ['server/tests/fixtures/schema-snapshots/baseline.sql', ['rust']],
    ['Cargo.lock', ['rust']],
    ['server/tests/fixtures/schema-snapshots/README.md', ['docs', 'rust']],
    ['server/fixtures/data.json', ['docs', 'rust']],
  ])('%s -> %j', (path, expected) => {
    expect(scopesOf(path)).toEqual(expected);
  });

  it.each([
    'justfile',
    '.github/workflows/ci.yml',
    '.claude/hooks/ci-before-push.sh',
    '.claude/skills/verify/verify.mjs',
    '.claude/settings.json',
    'scripts/changed-scopes.mjs',
    'scripts/migration-smoke-test.sh',
    'dprint.jsonc',
    '.typos.toml',
    '.gitignore',
    'deploy/fewd.service',
    '.env.example',
    'server/static/app.js',
    'server/tests/helper.test.mts',
  ])('%s matches no scope and selects every gate', (path) => {
    expect(scopesOf(path)).toEqual(['all']);
  });

  it('keeps a non-markdown file under docs/ out of the docs scope', () => {
    expect(scopesOf('docs/diagram.json')).toEqual(['all']);
    expect(scopesOf('docs/guide.md')).toEqual(['docs']);
  });

  it('unions the scopes of a mixed docs and Rust change', () => {
    const result = classifyPaths(['README.md', 'server/src/lib.rs', 'docs/a.md']);
    expect(result.scopes).toEqual(['docs', 'rust']);
    expect(result.triggers).toEqual({ docs: ['README.md', 'docs/a.md'], rust: ['server/src/lib.rs'] });
  });

  it('keeps the narrower scopes alongside all', () => {
    expect(scopesOf('src/a.ts', 'justfile')).toEqual(['frontend', 'all']);
  });

  it('returns no scopes for no paths, ignoring blanks and a ./ prefix', () => {
    expect(scopesOf()).toEqual([]);
    expect(scopesOf('', '')).toEqual([]);
    expect(scopesOf('./src/a.ts')).toEqual(['frontend']);
  });
});

describe('gate selection', () => {
  it('selects only dprint and typos for docs', () => {
    const ids = Object.keys(GATE_SCOPES).filter((id) => gateApplies(id, ['docs']));
    expect(ids.sort()).toEqual(['dprint', 'typos']);
  });

  // dprint.jsonc includes only ts/tsx/js/jsx/json/md. Every such path under
  // server/ also takes the docs scope, or no scope at all, so a rust-only
  // change has nothing for dprint to check.
  it('runs dprint over only the extensions the scope rules assume', () => {
    const config = readFileSync(join(HERE, '../dprint.jsonc'), 'utf8');
    expect(config).toContain('"**/*.{ts,tsx,js,jsx,json,md}"');
  });

  it('skips dprint for a Cargo.toml-only change', () => {
    expect(scopesOf('Cargo.toml')).toEqual(['rust']);
    expect(gateApplies('dprint', ['rust'])).toBe(false);
  });

  it('runs every gate, known or not, for all', () => {
    expect(Object.keys(GATE_SCOPES).every((id) => gateApplies(id, ['all']))).toBe(true);
    expect(gateApplies('not-in-the-table', ['docs'])).toBe(true);
  });

  it('gives every scope a gate and names only known scopes', () => {
    const selectors = new Set(Object.values(GATE_SCOPES).flat());
    for (const s of SCOPES) expect(selectors).toContain(s);
    for (const s of selectors) expect([...SCOPES, 'always']).toContain(s);
    expect(GATE_SCOPES.typos).toEqual(['always']);
  });

  it('covers exactly the gates verify.mjs defines', () => {
    expect(Object.keys(GATE_SCOPES).sort()).toEqual(GATES.map((g) => g.id).sort());
  });

  // verify.mjs creates a placeholder dist/ before any gate that compiles the
  // crate embedding it. That set is the gates the rust scope alone selects.
  it('marks every rust-only gate as a Rust gate in verify.mjs', () => {
    const rustOnly = Object.keys(GATE_SCOPES).filter((id) => GATE_SCOPES[id].every((s) => s === 'rust'));
    expect(GATES.filter((g) => g.rust).map((g) => g.id).sort()).toEqual(rustOnly.sort());
  });

  it('formats GitHub outputs, with all setting every scope', () => {
    expect(formatGithub(['docs'])).toBe('docs=true\nfrontend=false\nrust=false\n');
    expect(formatGithub(['docs', 'all'])).toBe('docs=true\nfrontend=true\nrust=true\n');
    expect(formatGithub([])).toBe('docs=false\nfrontend=false\nrust=false\n');
  });
});

describe('changedPaths', () => {
  const dirs = [];
  afterEach(() => {
    for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true });
  });

  // Isolate the temp repos from the user's git configuration, such as commit
  // signing or a different default branch.
  const env = { ...process.env, GIT_CONFIG_GLOBAL: '/dev/null', GIT_CONFIG_NOSYSTEM: '1' };
  const git = (cwd, ...args) =>
    execFileSync('git', ['-c', 'user.name=t', '-c', 'user.email=t@t', '-c', 'commit.gpgsign=false', ...args], {
      cwd,
      env,
      encoding: 'utf8',
    });
  const write = (repo, path, text = 'x\n') => {
    mkdirSync(dirname(join(repo, path)), { recursive: true });
    writeFileSync(join(repo, path), text);
  };

  function repoWithBranch() {
    const repo = realpathSync(mkdtempSync(join(tmpdir(), 'changed-scopes-')));
    dirs.push(repo);
    git(repo, 'init', '-q', '-b', 'main');
    write(repo, 'README.md');
    write(repo, 'src/a.ts');
    write(repo, 'old.rs');
    git(repo, 'add', '-A');
    git(repo, 'commit', '-q', '-m', 'init');
    git(repo, 'checkout', '-q', '-b', 'feat');
    return repo;
  }

  it('unions committed, renamed, staged, unstaged and untracked paths', () => {
    const repo = repoWithBranch();
    write(repo, 'server/x.rs');
    git(repo, 'add', '-A');
    git(repo, 'commit', '-q', '-m', 'add');
    mkdirSync(join(repo, 'server/src'), { recursive: true });
    git(repo, 'mv', 'old.rs', 'server/src/new.rs');
    git(repo, 'commit', '-q', '-m', 'rename');
    write(repo, 'docs/staged.md');
    git(repo, 'add', 'docs/staged.md');
    write(repo, 'src/a.ts', 'changed\n');
    write(repo, 'public/with space.svg');

    expect(changedPaths({ base: 'main', cwd: repo })).toEqual([
      'docs/staged.md',
      'old.rs',
      'public/with space.svg',
      'server/src/new.rs',
      'server/x.rs',
      'src/a.ts',
    ]);
    expect(changedPaths({ base: 'main', cwd: repo, worktree: false })).toEqual([
      'old.rs',
      'server/src/new.rs',
      'server/x.rs',
    ]);
  });

  it('reports root-relative paths from a subdirectory', () => {
    const repo = repoWithBranch();
    write(repo, 'src/untracked.ts');
    expect(changedPaths({ base: 'main', cwd: join(repo, 'src') })).toEqual(['src/untracked.ts']);
  });

  it('returns an empty list when nothing changed', () => {
    const repo = repoWithBranch();
    expect(changedPaths({ base: 'main', cwd: repo })).toEqual([]);
  });

  it('returns null for an unknown base or a directory outside a repository', () => {
    const repo = repoWithBranch();
    expect(changedPaths({ base: 'no-such-ref', cwd: repo })).toBeNull();
    const plain = realpathSync(mkdtempSync(join(tmpdir(), 'changed-scopes-plain-')));
    dirs.push(plain);
    expect(changedPaths({ base: 'main', cwd: plain })).toBeNull();
  });

  describe('CLI', () => {
    const run = (cwd, args, input) =>
      spawnSync(process.execPath, [CLI, ...args], { cwd, env, input, encoding: 'utf8' });
    // Each text-format line is a scope name indented ahead of its paths.
    const scopeLines = (r) => r.stdout.trim().split('\n').map((line) => line.trim().split(/\s+/)[0]);

    it('selects every gate and exits 0 when the base is unknown', () => {
      const repo = repoWithBranch();
      const r = run(repo, ['--base', 'no-such-ref', '--format', 'github']);
      expect(r.status).toBe(0);
      expect(r.stdout).toBe('docs=true\nfrontend=true\nrust=true\n');
      expect(r.stderr).toContain('could not list changes');
    });

    it('names each scope and says when nothing changed', () => {
      const repo = repoWithBranch();
      expect(run(repo, ['--base', 'main']).stdout).toContain('no changed files');
      write(repo, 'README.md', 'edited\n');
      write(repo, 'server/a.rs');
      expect(scopeLines(run(repo, ['--base', 'main']))).toEqual(['docs', 'rust']);
    });

    it('classifies NUL- or newline-separated paths from stdin', () => {
      const r = run(HERE, ['--stdin'], 'README.md\0justfile\n');
      expect(scopeLines(r)).toEqual(['docs', 'all']);
      expect(r.stdout).toContain('justfile');
    });

    it('exits 2 on a usage error', () => {
      expect(run(HERE, ['--bogus']).status).toBe(2);
      expect(run(HERE, ['--format', 'lines']).status).toBe(2);
      expect(run(HERE, ['--base']).status).toBe(2);
    });
  });
});

// GATE_SCOPES decides which gates a change selects, and ci.yml has to select
// the same ones. Nothing but these assertions keeps the two in step.
describe('ci.yml', () => {
  const workflow = readFileSync(join(HERE, '../.github/workflows/ci.yml'), 'utf8');
  const envJson = (name) => JSON.parse(workflow.match(new RegExp(`^\\s*${name}: '(.*)'$`, 'm'))[1]);

  // The CI-tier gates each job runs. verify.mjs runs the same ids locally.
  const JOB_GATES = {
    'rust-checks': ['fmt', 'clippy', 'mig-clippy', 'rust-test', 'mig-test', 'migration'],
    'frontend-checks': ['dprint', 'lockfile', 'lint', 'fe-test'],
    'typos-check': ['typos'],
  };

  it('runs every CI-tier gate in exactly one job', () => {
    const ids = Object.values(JOB_GATES).flat();
    expect(ids.sort()).toEqual(GATES.filter((g) => g.tier === 'ci').map((g) => g.id).sort());
  });

  it('publishes exactly the scopes the classifier defines', () => {
    const outputs = [...workflow.matchAll(/^ {6}(\w+): \$\{\{ steps\.scopes\.outputs\.(\w+) \}\}$/gm)];
    expect(outputs.map((m) => m[1])).toEqual([...SCOPES]);
    for (const [, output, step] of outputs) expect(output).toBe(step);
  });

  it('names only known scopes in its conditions', () => {
    for (const [, scope] of workflow.matchAll(/needs\.changes\.outputs\.(\w+)/g)) expect(SCOPES).toContain(scope);
  });

  it('gates each job on the scopes its own gates select', () => {
    const gated = envJson('GATED');
    for (const [job, scopes] of Object.entries(gated)) {
      expect(new Set(scopes)).toEqual(new Set(JOB_GATES[job].flatMap((id) => GATE_SCOPES[id])));
    }
    const always = envJson('ALWAYS');
    for (const job of always.filter((j) => JOB_GATES[j])) {
      expect(JOB_GATES[job].flatMap((id) => GATE_SCOPES[id])).toContain('always');
    }
    // `changes` produces the scopes rather than consuming them, so it is the
    // one job in ALWAYS that runs no gate.
    expect([...Object.keys(gated), ...always].sort()).toEqual([...Object.keys(JOB_GATES), 'changes'].sort());
  });
});
