"""Refresh the complete, pinned official AnimateIcons catalog for Flutter."""
import base64
import hashlib
import json
import shutil
import subprocess
import tarfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSION = '0.1.0'
INTEGRITY = 'o556Pw+B4OGShwhCBNiAs58e0yUXwb4cSipfy803Mf+UVmkSa+NV8Wmv8ZhdkDF7B6A7cl7E9jVbnHGBhRC59w=='
URL = f'https://registry.npmjs.org/@animateicons/vanilla/-/vanilla-{VERSION}.tgz'


def main():
    cache = ROOT / 'output/vendor/animateicons-package'
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / f'vanilla-{VERSION}.tgz'
    data = urllib.request.urlopen(URL, timeout=30).read()
    if base64.b64encode(hashlib.sha512(data).digest()).decode() != INTEGRITY:
        raise ValueError('The package does not match its pinned npm integrity.')
    archive.write_bytes(data)
    with tarfile.open(archive) as package:
        package.extractall(cache, filter='data')
    library = ROOT / 'flutter_app/packages/animate_icons'
    (library / 'assets').mkdir(parents=True, exist_ok=True)
    node = shutil.which('node')
    if node is None:
        raise FileNotFoundError('Node.js is required to read the upstream data exports.')
    subprocess.run([node, '--input-type=module', '-e', r'''
import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
const [source, target] = process.argv.slice(1);
const catalog = {};
for (const library of ['lucide', 'huge']) {
  const exports = await import(pathToFileURL(path.join(source, 'dist', library + '.js')));
  for (const icon of Object.values(exports)) catalog[library + '/' + icon.name] = icon;
}
fs.writeFileSync(target, JSON.stringify(catalog, (_, value) =>
  typeof value === 'number' && !Number.isFinite(value) ? null : value));
console.log(`AnimateIcons: ${Object.keys(catalog).length} upstream icons`);
''', str(cache / 'package'), str(library / 'assets/catalog.json')], check=True)
    for source, target in [('LICENSE', 'LICENSE'), ('THIRD_PARTY_NOTICES.md', 'THIRD_PARTY_NOTICES.md')]:
        shutil.copyfile(cache / 'package' / source, library / target)
    shutil.copyfile(library / 'LICENSE', ROOT / 'resources/licenses/ANIMATEICONS-LICENSE.txt')
    shutil.copyfile(library / 'THIRD_PARTY_NOTICES.md', ROOT / 'resources/licenses/ANIMATEICONS-THIRD-PARTY-NOTICES.txt')
    catalog_bytes = (library / 'assets/catalog.json').read_bytes()
    catalog = json.loads(catalog_bytes)
    (ROOT / 'resources/icons/flutter-sources.json').write_text(json.dumps({
        'package': '@animateicons/vanilla', 'version': VERSION,
        'source': URL, 'integrity': 'sha512-' + INTEGRITY,
        'repository': 'https://github.com/Avijit07x/animateicons',
        'iconCount': len(catalog), 'catalogSha256': hashlib.sha256(catalog_bytes).hexdigest(),
        'adapter': 'flutter_app/packages/animate_icons/lib/animate_icons.dart',
        'refresh': 'scripts/update-animateicons.py',
    }, indent=2), encoding='utf-8')


if __name__ == '__main__':
    main()
