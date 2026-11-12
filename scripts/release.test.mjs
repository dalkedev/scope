import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync, chmodSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, delimiter } from 'node:path';
import { preparePublicResources, verifyPublicResources } from './public-resources.mjs';
import { spawnSync } from 'node:child_process';
import { verifyReleaseCommit } from './release-ref.mjs';

test('review proposal modes only open draft PRs with validated branches and committed bodies', () => {
  const workflow = readFileSync(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8');
  assert.match(workflow, /proposal:\n[\s\S]*?if: inputs\.mode == 'proposal' \|\| inputs\.mode == 'documentation'/);
  assert.match(workflow, /\n  build:\n    if: inputs\.mode == 'draft'\n/);
  const block = workflow.match(/name: Open draft pull request as the Actions bot\n        shell: bash\n        run: \|\n([\s\S]*?)\n  build:/)?.[1];
  assert.ok(block, 'proposal shell is present');
  const script = block.split('\n').map((line) => line.slice(10)).join('\n');
  const root = mkdtempSync(join(tmpdir(), 'snurpo-review-proposal-'));
  try {
    mkdirSync(join(root, 'bin'));
    mkdirSync(join(root, 'docs/website'), { recursive: true });
    mkdirSync(join(root, 'docs/releases'), { recursive: true });
    writeFileSync(join(root, 'docs/website/REVIEW.md'), 'Documentation review');
    writeFileSync(join(root, 'docs/releases/1.5.0-review.md'), 'Release review');
    writeFileSync(join(root, 'proposal.sh'), script);
    const fakeGh = join(root, 'bin/gh');
    const fakeGhScript = join(root, 'fake-gh.cjs');
    const log = join(root, 'gh-calls.jsonl');
    writeFileSync(fakeGh, '#!/usr/bin/env bash\nnode "$REVIEW_TEST_GH_SCRIPT" "$@"\n');
    writeFileSync(fakeGhScript, `const fs = require('node:fs');\nconst args = process.argv.slice(2);\nfs.appendFileSync(process.env.REVIEW_TEST_LOG, JSON.stringify(args) + '\\n');\nif (args[0] === 'pr' && args[1] === 'list') process.stdout.write(process.env.REVIEW_TEST_EXISTING || '');\nelse if (!(args[0] === 'pr' && args[1] === 'create')) process.exit(8);\n`);
    chmodSync(fakeGh, 0o755);
    const run = (mode, branch, tag = 'v1.5.0', existing = '') => {
      writeFileSync(log, '');
      const result = spawnSync('bash', ['--noprofile', '--norc', '-eo', 'pipefail', 'proposal.sh'], {
        cwd: root,
        encoding: 'utf8',
        env: { ...process.env, PATH: [join(root, 'bin'), process.env.PATH].join(delimiter), PROPOSAL_MODE: mode,
          PROPOSAL_BRANCH: branch, RELEASE_TAG: tag, REVIEW_TEST_LOG: log, REVIEW_TEST_EXISTING: existing,
          REVIEW_TEST_GH_SCRIPT: fakeGhScript },
      });
      const calls = readFileSync(log, 'utf8').trim().split('\n').filter(Boolean).map((line) => JSON.parse(line));
      return { ...result, calls };
    };

    const docs = run('documentation', 'docs/website-guides', 'unused');
    assert.equal(docs.status, 0, docs.stderr);
    assert.deepEqual(docs.calls.at(-1), ['pr', 'create', '--draft', '--base', 'main', '--head', 'docs/website-guides',
      '--title', 'Snurpo website documentation: maintainer review', '--body-file', 'docs/website/REVIEW.md']);
    const release = run('proposal', 'release/1.5.0');
    assert.equal(release.status, 0, release.stderr);
    assert.equal(release.calls.at(-1).at(-1), 'docs/releases/1.5.0-review.md');

    for (const input of [
      ['documentation', 'main'], ['documentation', 'docs/../main'], ['draft', 'docs/website-guides'],
      ['proposal', 'release/1.4.2'], ['proposal', 'release/1.5.0', 'invalid'],
    ]) {
      const invalid = run(...input);
      assert.notEqual(invalid.status, 0, JSON.stringify(input));
      assert.deepEqual(invalid.calls, [], 'invalid proposal makes no GitHub call');
    }
    const duplicate = run('documentation', 'docs/website-guides', 'unused', '43');
    assert.equal(duplicate.status, 0, duplicate.stderr);
    assert.equal(duplicate.calls.length, 1);
    assert.match(duplicate.stdout, /already exists: #43/);

    rmSync(join(root, 'docs/website/REVIEW.md'));
    const missing = run('documentation', 'docs/website-guides');
    assert.notEqual(missing.status, 0);
    assert.deepEqual(missing.calls, []);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('release assets require an exact existing tag, matching HEAD and clean tracked sources', () => {
  const root = mkdtempSync(join(tmpdir(), 'scope-release-ref-'));
  const git = (...args) => {
    const result = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  try {
    git('init');
    git('config', 'user.name', 'Snurpo release fixture');
    git('config', 'user.email', 'fixture@example.test');
    writeFileSync(join(root, 'source.txt'), 'reviewed source');
    git('add', 'source.txt');
    git('commit', '-m', 'fixture baseline');
    git('branch', 'v1.0.0');
    assert.throws(() => verifyReleaseCommit(root, 'v1.0.0'), /existing Git tag/);
    git('tag', '-a', 'v1.0.0', '-m', 'reviewed tag');
    assert.equal(verifyReleaseCommit(root, 'v1.0.0'), git('rev-parse', 'HEAD'));
    writeFileSync(join(root, 'untracked-source.txt'), 'not in the source archive');
    assert.throws(() => verifyReleaseCommit(root, 'v1.0.0'), /including untracked files/);
    rmSync(join(root, 'untracked-source.txt'));
    writeFileSync(join(root, 'source.txt'), 'unreviewed edit');
    assert.throws(() => verifyReleaseCommit(root, 'v1.0.0'), /must be clean/);
    git('add', 'source.txt');
    git('commit', '-m', 'different commit');
    assert.throws(() => verifyReleaseCommit(root, 'v1.0.0'), /checked-out commit/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('public staging excludes optional native Frida while preserving local binaries and source setup', () => {
  const root = mkdtempSync(join(tmpdir(), 'scope-release-'));
  try {
    mkdirSync(join(root, 'resources/bin'), { recursive: true });
    mkdirSync(join(root, 'resources/device'), { recursive: true });
    for (const name of [
      'scope-frida-worker',
      'scope-frida-worker.exe',
      'frida-server-17.9.1-android-arm64',
      'scope-mcp',
    ]) {
      writeFileSync(join(root, 'resources/bin', name), name);
    }
    writeFileSync(join(root, 'resources/device/install-runtime.py'), 'source setup');
    const config = preparePublicResources(root);
    const patch = JSON.parse(readFileSync(config));
    assert.equal(patch.bundle.resources['../resources/'], null);
    const stage = join(root, '.runtime/public-resources');
    assert(existsSync(join(stage, 'bin/scope-mcp')));
    assert(!existsSync(join(stage, 'bin/scope-frida-worker')));
    assert(!existsSync(join(stage, 'bin/frida-server-17.9.1-android-arm64')));
    assert(existsSync(join(root, 'resources/bin/scope-frida-worker')));
    assert(verifyPublicResources(stage).includes(join('device', 'install-runtime.py')));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('release resource verification rejects accidentally included private session data', () => {
  const root = mkdtempSync(join(tmpdir(), 'scope-release-secret-'));
  try {
    writeFileSync(join(root, 'install-runtime.py'), 'source');
    writeFileSync(join(root, 'scope-ca-key.pem'), 'not a real key');
    assert.throws(() => verifyPublicResources(root), /Private runtime data/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
