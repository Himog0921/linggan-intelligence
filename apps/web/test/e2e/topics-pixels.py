"""Exact private screenshot comparison. Requires Pillow; never modifies originals."""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image, ImageChops

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--baseline', type=Path, required=True)
parser.add_argument('--candidate', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
if args.output.resolve() in (args.baseline.resolve(), args.candidate.resolve()):
    parser.error('Output must differ from both original directories')
args.output.mkdir(parents=True, exist_ok=False)
baseline = {p.name: p for p in args.baseline.glob('*.png')}
candidate = {p.name: p for p in args.candidate.glob('*.png')}
if not baseline or not candidate:
    parser.error('Both original directories must contain PNG screenshots')
report = dict(baselineCount=len(baseline), candidateCount=len(candidate),
              missing=sorted(baseline.keys() - candidate.keys()),
              extra=sorted(candidate.keys() - baseline.keys()),
              masks=0, tolerance=0, cropping=False, images=[])
for name in sorted(baseline.keys() & candidate.keys()):
    with Image.open(baseline[name]) as original, Image.open(candidate[name]) as current:
        row = dict(name=name, baselineSize=original.size, candidateSize=current.size)
        if original.size != current.size:
            row['dimensionMismatch'] = True
        elif hashlib.sha256(baseline[name].read_bytes()).digest() == hashlib.sha256(candidate[name].read_bytes()).digest():
            row.update(changedPixels=0, bbox=None)
        else:
            diff = ImageChops.difference(original.convert('RGB'), current.convert('RGB'))
            maximum = ImageChops.lighter(ImageChops.lighter(diff.getchannel('R'), diff.getchannel('G')), diff.getchannel('B'))
            row.update(changedPixels=diff.width * diff.height - maximum.histogram()[0], bbox=diff.getbbox())
            if row['changedPixels']:
                diff.save(args.output / name)
        report['images'].append(row)
report['exact'] = sum(row.get('changedPixels') == 0 for row in report['images'])
report['different'] = len(report['images']) - report['exact']
(args.output / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2))
print(json.dumps({k: v for k, v in report.items() if k != 'images'}, ensure_ascii=False))
raise SystemExit(bool(report['different'] or report['missing'] or report['extra']))
