// @vitest-environment node
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it } from 'vitest';
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

  it('runs dprint for a Cargo.toml-only change', () => {
    expect(scopesOf('Cargo.toml')).toEqual(['rust']);
    expect(gateApplies('dprint', ['rust'])).toBe(true);
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
    const source = readFileSync(join(HERE, '../.claude/skills/verify/verify.mjs'), 'utf8');
    const ids = [...source.matchAll(/\{ id: '([^']+)'/g)].map((m) => m[1]);
    expect(ids.length).toBeGreaterThan(0);
    expect(Object.keys(GATE_SCOPES).sort()).toEqual(ids.sort());
  });

  it('formats GitHub outputs, with all setting every scope', () => {
    expect(formatGithub(['docs'])).toBe('docs=true\nfrontend=false\nrust=false\nall=false\n');
    expect(formatGithub(['docs', 'all'])).toBe('docs=true\nfrontend=true\nrust=true\nall=true\n');
    expect(formatGithub([])).toBe('docs=false\nfrontend=false\nrust=false\nall=false\n');
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

    it('selects every gate and exits 0 when the base is unknown', () => {
      const repo = repoWithBranch();
      const r = run(repo, ['--base', 'no-such-ref', '--format', 'github']);
      expect(r.status).toBe(0);
      expect(r.stdout).toBe('docs=true\nfrontend=true\nrust=true\nall=true\n');
      expect(r.stderr).toContain('could not list changes');
    });

    it('prints one scope per line, and none for no changes', () => {
      const repo = repoWithBranch();
      expect(run(repo, ['--base', 'main', '--format', 'lines']).stdout).toBe('none\n');
      write(repo, 'README.md', 'edited\n');
      write(repo, 'server/a.rs');
      expect(run(repo, ['--base', 'main', '--format', 'lines']).stdout).toBe('docs\nrust\n');
    });

    it('classifies NUL- or newline-separated paths from stdin', () => {
      const r = run(HERE, ['--stdin', '--format', 'lines'], 'README.md\0justfile\n');
      expect(r.stdout).toBe('docs\nall\n');
    });

    it('exits 2 on a usage error', () => {
      expect(run(HERE, ['--bogus']).status).toBe(2);
      expect(run(HERE, ['--format', 'yaml']).status).toBe(2);
      expect(run(HERE, ['--base']).status).toBe(2);
    });
  });
});
