import fs from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { bundleProvider,brandRuntime } from '../media-provider/build.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
// Pin an active Node LTS release and verify every downloaded/cached runtime against
// the publisher's SHA256 before packaging. The release app never uses PATH Node.
const version='v24.21.0';
import { execFileSync } from 'node:child_process';
const isWin = process.platform === 'win32';
const platform = isWin ? 'win-x64' : 'linux-x64';
const sha256 = isWin
  ? 'ba4e6d110e8c1592a1ecd390f6b05f3da124b13871a5be62b341a07a853c6c32'
  : 'fd8e59d5a511510f6a298afb548f18c7d2b1be404d8b4a27d94fbe49f56cb2d6';
const runtime = path.join(root, 'resources', 'runtime');
await fs.mkdir(runtime, { recursive: true });
const executable = path.join(runtime, isWin ? 'soundflow-node.exe' : 'soundflow-node');
const digest = buffer => createHash('sha256').update(buffer).digest('hex');
const icon = path.join(root, 'resources/icon.ico');
const iconHash = digest(await fs.readFile(icon));
const record = await fs.readFile(path.join(runtime, 'runtime.json'), 'utf8').then(JSON.parse).catch(() => null);
let binary = await fs.readFile(executable).catch(() => null);
const alreadyBranded = binary && record?.source_sha256 === sha256 && record?.platform === platform && record?.brand_version === '3.2.0';

if (!alreadyBranded && (!binary || record?.source_sha256 !== sha256)) {
  if (isWin) {
    console.error(`Fetching official Node.js ${version} Windows x64 runtime…`);
    const response = await fetch(`https://nodejs.org/dist/${version}/win-x64/node.exe`, { signal: AbortSignal.timeout(120000) });
    if (!response.ok) throw new Error(`Node runtime download failed HTTP ${response.status}`);
    binary = Buffer.from(await response.arrayBuffer());
    if (digest(binary) !== sha256) throw new Error('Node runtime SHA256 did not match the pinned official release.');
    await fs.writeFile(`${executable}.part`, binary);
    await fs.rename(`${executable}.part`, executable);
  } else {
    console.error(`Fetching official Node.js ${version} Linux x64 runtime…`);
    const url = `https://nodejs.org/dist/${version}/node-${version}-linux-x64.tar.xz`;
    const response = await fetch(url, { signal: AbortSignal.timeout(120000) });
    if (!response.ok) throw new Error(`Node runtime download failed HTTP ${response.status}`);
    const archive = Buffer.from(await response.arrayBuffer());
    if (digest(archive) !== sha256) throw new Error('Node runtime SHA256 did not match the pinned official release.');
    const tarFile = path.join(runtime, 'node.tar.xz');
    await fs.writeFile(tarFile, archive);
    execFileSync('tar', ['-xJ', '-C', runtime, '--strip-components=2', '-f', tarFile, `node-${version}-linux-x64/bin/node`]);
    await fs.rename(path.join(runtime, 'node'), executable);
    await fs.chmod(executable, 0o755);
    await fs.unlink(tarFile).catch(() => null);
    binary = await fs.readFile(executable);
  }
}
if (!alreadyBranded && isWin) await brandRuntime(executable, icon);
const licenseFile = path.join(runtime, 'NODE-LICENSE.txt');
if (!await fs.access(licenseFile).then(() => true).catch(() => false)) {
  const licenseResponse = await fetch(`https://raw.githubusercontent.com/nodejs/node/${version}/LICENSE`, { signal: AbortSignal.timeout(30000) });
  if (!licenseResponse.ok) throw new Error('Could not fetch pinned Node.js distribution license.');
  await fs.writeFile(licenseFile, await licenseResponse.text());
}
await fs.writeFile(
  path.join(runtime, 'runtime.json'),
  JSON.stringify({
    name: 'SoundFlow Media',
    engine: 'Node.js',
    version,
    platform,
    source_sha256: sha256,
    sha256: digest(await fs.readFile(executable)),
    icon_sha256: iconHash,
    brand_version: '3.2.0',
    source: isWin ? `https://nodejs.org/dist/${version}/win-x64/node.exe` : `https://nodejs.org/dist/${version}/node-${version}-linux-x64.tar.xz`,
  }, null, 2) + '\n'
);
await bundleProvider();
console.error(`Bundled runtime: Node.js ${version}; no Python, FFmpeg or system Node dependency.`);
