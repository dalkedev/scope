import { spawnSync } from 'node:child_process';
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from 'node:fs';
import { createHash } from 'node:crypto';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifyPublicResources } from './public-resources.mjs';
import { verifyReleaseCommit } from './release-ref.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const tag = process.argv[2];
if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag || ''))
  throw new Error('Expected a version tag, e.g. v0.1.0-alpha.1');
const output = resolve(process.argv[3] || join(root, '.runtime/release-assets'));
mkdirSync(output, { recursive: true });
function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, encoding: 'utf8' });
  if (result.status !== 0)
    throw new Error(`${command} failed: ${result.stderr || result.error || result.stdout}`);
  return result.stdout.trim();
}
const commit = verifyReleaseCommit(root, tag);
verifyPublicResources(join(root, '.runtime/public-resources'));
const platform =
  process.platform === 'darwin' ? 'macOS' : process.platform === 'win32' ? 'Windows' : 'Linux';
const arch = process.arch;
const bundle = join(root, 'target/release/bundle');
const assets = [];
function copyInstallers(subdir, extension, suffix) {
  const directory = join(bundle, subdir);
  if (!existsSync(directory)) throw new Error(`Missing installer directory: ${directory}`);
  const installers = readdirSync(directory).filter((name) => name.endsWith(extension));
  if (installers.length !== 1)
    throw new Error(`Expected exactly one ${extension} installer, found ${installers.length}`);
  const destination = join(output, `Snurpo-${tag}-${platform}-${arch}${suffix}`);
  copyFileSync(join(directory, installers[0]), destination);
  assets.push(destination);
}
if (process.platform === 'darwin') {
  const app = join(bundle, 'macos/Snurpo.app');
  verifyPublicResources(join(app, 'Contents/Resources/resources'));
  if (
    !existsSync(join(app, 'Contents/Resources/LICENSE')) ||
    !existsSync(join(app, 'Contents/Resources/THIRD_PARTY_NOTICES.md'))
  ) {
    throw new Error('The application must include the Snurpo license and third-party notice index');
  }
  const destination = join(output, `Snurpo-${tag}-${platform}-${arch}.zip`);
  run('/usr/bin/ditto', ['-c', '-k', '--sequesterRsrc', '--keepParent', app, destination]);
  assets.push(destination);
  const source = join(output, `Snurpo-${tag}-source.zip`);
  run('git', ['archive', '--format=zip', `--prefix=snurpo-${tag}/`, `--output=${source}`, commit]);
  assets.push(source);
} else if (process.platform === 'win32') copyInstallers('nsis', '.exe', '-setup.exe');
else {
  copyInstallers('appimage', '.AppImage', '.AppImage');
  copyInstallers('deb', '.deb', '.deb');
}
const hashes =
  assets
    .map(
      (path) =>
        `${createHash('sha256').update(readFileSync(path)).digest('hex')}  ${basename(path)}`,
    )
    .join('\n') + '\n';
writeFileSync(join(output, `${platform}-${arch}-SHA256SUMS.txt`), hashes);
writeFileSync(
  join(output, `${platform}-${arch}-build.json`),
  JSON.stringify(
    {
      tag,
      commit,
      platform,
      arch,
      publicFridaBinariesIncluded: false,
      assets: assets.map((path) => basename(path)),
    },
    null,
    2,
  ) + '\n',
);
console.log(hashes);
