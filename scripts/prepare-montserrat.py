"""Export the pinned Montserrat heading faces bundled with SoundFlow."""
from pathlib import Path
import hashlib
import json
import shutil
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

root = Path(__file__).resolve().parent.parent
source = root / 'resources/fonts/Montserrat-Variable.ttf'
data = source.read_bytes()
blob = hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()
if blob != 'c97aca18592834d8706549c279cb8d5ac5d85f69':
    raise ValueError('Montserrat source differs from the pinned upstream font.')
for name, weight in [('SemiBold', 600), ('Bold', 700)]:
    face = instantiateVariableFont(TTFont(source), {'wght': weight}, updateFontNames=True)
    face.save(source.with_name(f'Montserrat-{name}.ttf'))
    target = root / 'flutter_app/assets/fonts'
    target.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source.with_name(f'Montserrat-{name}.ttf'), target / f'Montserrat-{name}.ttf')
(source.parent / 'montserrat-sources.json').write_text(json.dumps({
    'family': 'Montserrat',
    'source': 'https://raw.githubusercontent.com/google/fonts/main/ofl/montserrat/Montserrat%5Bwght%5D.ttf',
    'gitBlobSha1': blob,
    'sourceSha256': hashlib.sha256(data).hexdigest(),
    'license': '../licenses/MONTSERRAT-OFL.txt',
    'instances': {'Montserrat-SemiBold.ttf': 600, 'Montserrat-Bold.ttf': 700},
    'generator': 'scripts/prepare-montserrat.py (fonttools 4.60.1)',
}, indent=2), encoding='utf-8')
print('Pinned Montserrat verified; heading faces exported.')
