"""Create the two installer faces from the pinned OFL Onest variable source."""
from pathlib import Path
import hashlib
import json
import shutil
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

root = Path(__file__).resolve().parent.parent
source = root / 'resources/fonts/Onest-Variable.ttf'
data = source.read_bytes()
blob = hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()
if blob != '477cb31027450d80e486a53119641bf87dde3d4c':
    raise ValueError('Onest source differs from the pinned upstream font.')
for name, weight in [('Regular', 400), ('SemiBold', 600)]:
    face = instantiateVariableFont(TTFont(source), {'wght': weight}, updateFontNames=True)
    face.save(source.with_name(f'Onest-{name}.ttf'))
    flutter_assets = root / 'flutter_app/assets/fonts'
    flutter_assets.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source.with_name(f'Onest-{name}.ttf'), flutter_assets / f'Onest-{name}.ttf')
(source.parent / 'sources.json').write_text(json.dumps({
    'family': 'Onest',
    'source': 'https://raw.githubusercontent.com/google/fonts/main/ofl/onest/Onest%5Bwght%5D.ttf',
    'gitBlobSha1': blob,
    'sourceSha256': hashlib.sha256(data).hexdigest(),
    'license': '../licenses/ONEST-OFL.txt',
    'instances': {'Onest-Regular.ttf': 400, 'Onest-SemiBold.ttf': 600},
    'generator': 'scripts/prepare-onest.py (fonttools 4.60.1)',
}, indent=2), encoding='utf-8')
print('Pinned Onest verified; Regular and SemiBold exported.')
