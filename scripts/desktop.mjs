import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, copyFileSync } from 'node:fs';
import { dirname, resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { preparePublicResources } from './public-resources.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const env = { ...process.env };
const mode = process.argv[2] || 'dev';
if (!['dev', 'build'].includes(mode)) throw new Error('Usage: node scripts/desktop.mjs dev|build');
const localHome = resolve(root, '../../work/toolchain');
const extension = process.platform === 'win32' ? '.exe' : '';
let cargo = `cargo${extension}`;
if (spawnSync(cargo, ['--version'], { env, stdio: 'ignore' }).status !== 0) {
  const candidates = [
    process.env.CARGO_HOME && join(process.env.CARGO_HOME, 'bin'),
    process.env.HOME && join(process.env.HOME, '.cargo/bin'),
    join(localHome, 'cargo/bin'),
  ].filter(Boolean);
  const bin = candidates.find((p) => existsSync(join(p, cargo)));
  if (!bin)
    throw new Error(
      'Rust is required. Install it from https://rustup.rs, then reopen your terminal.',
    );
  env.PATH = `${bin}${process.platform === 'win32' ? ';' : ':'}${env.PATH || ''}`;
  if (bin === join(localHome, 'cargo/bin')) {
    env.CARGO_HOME = join(localHome, 'cargo');
    env.RUSTUP_HOME = join(localHome, 'rustup');
  }
}
function run(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: root,
      env,
      stdio: 'inherit',
      shell: process.platform === 'win32',
    });
    child.on('error', reject);
    child.on('exit', (code) =>
      code === 0 ? resolve() : reject(new Error(`${command} exited with ${code}`)),
    );
  });
}
await run(cargo, [
  'build',
  '--locked',
  '-p',
  'scope-mcp',
  ...(mode === 'build' ? ['--release'] : []),
]);
const binDir = join(root, 'resources/bin');
mkdirSync(binDir, { recursive: true });
copyFileSync(
  join(root, 'target', mode === 'build' ? 'release' : 'debug', `scope-mcp${extension}`),
  join(binDir, `scope-mcp${extension}`),
);
const tauri = join(root, 'node_modules/.bin', process.platform === 'win32' ? 'tauri.cmd' : 'tauri');
const bundles =
  process.platform === 'darwin' ? 'app' : process.platform === 'win32' ? 'nsis' : 'appimage,deb';
const publicConfig = mode === 'build' ? preparePublicResources(root) : undefined;
await run(tauri, [
  mode,
  ...(mode === 'build' ? ['--bundles', bundles, '--config', publicConfig] : []),
]);
if (mode === 'build' && process.platform === 'darwin') {
  const application = join(root, 'target/release/bundle/macos/Snurpo.app');
  // Tauri may reuse an earlier bundle's resource envelope. Sign the complete
  // final app after resources are staged, then verify exactly what we ship.
  await run('/usr/bin/codesign', ['--force', '--deep', '--sign', '-', application]);
  await run('/usr/bin/codesign', ['--verify', '--deep', '--strict', application]);
}
