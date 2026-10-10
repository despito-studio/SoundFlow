import fs from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { bundleProvider,brandRuntime } from '../media-provider/build.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
// Pin an active Node LTS release and verify every downloaded/cached runtime against
// the publisher's SHA256 before packaging. The release app never uses PATH Node.
const version='v24.21.0';
const sha256='ba4e6d110e8c1592a1ecd390f6b05f3da124b13871a5be62b341a07a853c6c32';
const runtime=path.join(root,'resources','runtime');await fs.mkdir(runtime,{recursive:true});
const executable=path.join(runtime,'soundflow-node.exe');
const digest=buffer=>createHash('sha256').update(buffer).digest('hex');
const icon=path.join(root,'resources/icon.ico');const iconHash=digest(await fs.readFile(icon));
const record=await fs.readFile(path.join(runtime,'runtime.json'),'utf8').then(JSON.parse).catch(()=>null);
let binary=await fs.readFile(executable).catch(()=>null);
const alreadyBranded=binary && record?.source_sha256===sha256 && record?.sha256===digest(binary) && record?.icon_sha256===iconHash && record?.brand_version==='3.2.0';
if(!alreadyBranded && (!binary || digest(binary)!==sha256)) {
  console.error(`Fetching official Node.js ${version} Windows x64 runtime…`);
  const response=await fetch(`https://nodejs.org/dist/${version}/win-x64/node.exe`,{signal:AbortSignal.timeout(120000)});
  if(!response.ok)throw new Error(`Node runtime download failed HTTP ${response.status}`);
  binary=Buffer.from(await response.arrayBuffer());
  if(digest(binary)!==sha256)throw new Error('Node runtime SHA256 did not match the pinned official release.');
  await fs.writeFile(`${executable}.part`,binary);await fs.rename(`${executable}.part`,executable);
}
if(!alreadyBranded)await brandRuntime(executable,icon);
const licenseResponse=await fetch(`https://raw.githubusercontent.com/nodejs/node/${version}/LICENSE`,{signal:AbortSignal.timeout(30000)});
if(!licenseResponse.ok)throw new Error('Could not fetch pinned Node.js distribution license.');
await fs.writeFile(path.join(runtime,'NODE-LICENSE.txt'),await licenseResponse.text());
await fs.writeFile(path.join(runtime,'runtime.json'),JSON.stringify({name:'SoundFlow Media',engine:'Node.js',version,platform:'win-x64',source_sha256:sha256,sha256:digest(await fs.readFile(executable)),icon_sha256:iconHash,brand_version:'3.2.0',source:`https://nodejs.org/dist/${version}/win-x64/node.exe`},null,2)+'\n');
await bundleProvider();console.error(`Bundled runtime: Node.js ${version}; no Python, FFmpeg or system Node dependency.`);
