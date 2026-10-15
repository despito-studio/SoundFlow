import fs from 'node:fs/promises';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const outputPath = process.argv[2] || path.join(root, 'output/licenses/RUST-LICENSES.txt');
await fs.mkdir(path.dirname(outputPath), { recursive: true });

const metadataRaw = execFileSync('cargo', ['metadata', '--manifest-path', path.join(root, 'Cargo.toml'), '--format-version', '1', '--locked'], {
  encoding: 'utf8',
  maxBuffer: 64 * 1024 * 1024,
});

const metadata = JSON.parse(metadataRaw);
const sections = ['SoundFlow native Linux third-party notices. Slint is used under its Royalty-free Applications License 2.0; see SLINT-LICENSE.md and About SoundFlow.'];

const packages = (metadata.packages || [])
  .filter(p => p.source)
  .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));

for (const pkg of packages) {
  sections.push(`${pkg.name} ${pkg.version}\n${pkg.repository || ''}\nLicense expression: ${pkg.license || 'unknown'}`);
  const pkgDir = path.dirname(pkg.manifest_path);
  try {
    const files = await fs.readdir(pkgDir);
    const licenseFiles = files.filter(f => /^(LICENSE|COPYING|COPYRIGHT|NOTICE)([._-]|$)/i.test(f));
    for (const f of licenseFiles) {
      const content = await fs.readFile(path.join(pkgDir, f), 'utf8');
      sections.push(`${f}\n${content}`);
    }
  } catch {}
}

await fs.writeFile(outputPath, sections.join('\n\n--------------------\n\n'), 'utf8');
console.error(`Collected Rust licenses: ${outputPath}`);
