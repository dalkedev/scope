import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { preparePublicResources, verifyPublicResources } from './public-resources.mjs';
import { spawnSync } from 'node:child_process';
import { verifyReleaseCommit } from './release-ref.mjs';

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
