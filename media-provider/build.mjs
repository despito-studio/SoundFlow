import { build } from 'esbuild';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { rcedit } from 'rcedit';
export async function brandRuntime(executable,icon) {
  if (process.platform === 'win32') {
    await rcedit(executable,{icon,'product-version':'3.2.0','version-string':{ProductName:'SoundFlow',FileDescription:'SoundFlow Media',InternalName:'SoundFlow Media',OriginalFilename:'soundflow-node.exe',ProductVersion:'3.2.0'}});
  }
}
export async function bundleProvider() {
  const root=path.dirname(fileURLToPath(import.meta.url));const output=path.join(root,'dist');await fs.mkdir(output,{recursive:true});
  await build({entryPoints:[path.join(root,'worker.mjs')],outfile:path.join(output,'worker.cjs'),bundle:true,format:'cjs',platform:'node',target:'node24',minify:true,keepNames:true,legalComments:'eof',define:{__SOUNDFLOW_BUNDLED__:'true'},logOverride:{'empty-import-meta':'silent'},logLevel:'warning'});
  const packages=['youtubei.js','fflate','meriyah','@bufbuild/protobuf'];const sections=[];
  for(const name of packages) {
    const dir=path.join(root,'node_modules',name);const metadata=JSON.parse(await fs.readFile(path.join(dir,'package.json'),'utf8'));
    const files=await fs.readdir(dir);const license=files.find(f=>/^licen[sc]e(?:\.|$)/i.test(f));
    let contents;
    if(license)contents=await fs.readFile(path.join(dir,license),'utf8');
    else if(name==='@bufbuild/protobuf') {
      const cached=path.join(root,'licenses',`protobuf-${metadata.version}.txt`);
      contents=await fs.readFile(cached,'utf8').catch(()=>null);
      if(!contents) {
        const response=await fetch(`https://raw.githubusercontent.com/bufbuild/protobuf-es/v${metadata.version}/LICENSE`,{signal:AbortSignal.timeout(30000)});
        if(!response.ok)throw new Error('Missing pinned protobuf license.');
        const apache=await response.text();const varint=await fs.readFile(path.join(dir,'dist/esm/wire/varint.js'),'utf8');
        contents=`${apache}\n\nGoogle Protobuf varint BSD attribution:\n${varint.slice(0,varint.indexOf('/**')).replace(/^\/\/ ?/gm,'')}`;
        await fs.mkdir(path.dirname(cached),{recursive:true});await fs.writeFile(cached,contents);
      }
    } else throw new Error(`Missing distribution license for ${name}`);
    sections.push(`${name} ${metadata.version}\n${metadata.repository?.url||metadata.repository||''}\n${contents}`);
  }
  await fs.writeFile(path.join(output,'THIRD-PARTY-LICENSES.txt'),sections.join('\n\n--------------------\n\n'));
  console.error(`Provider bundle: ${path.join(output,'worker.cjs')}`);
}
if(process.argv[1] && path.resolve(process.argv[1])===fileURLToPath(import.meta.url))await bundleProvider();
