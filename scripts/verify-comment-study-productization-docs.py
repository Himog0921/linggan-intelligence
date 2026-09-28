#!/usr/bin/env python3
"""Check the imported Comment Study contract; never touches DB, network or models."""
import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

MANUAL_SHA256 = {
    'docs/agents/comment-study-productization-001-handoff.md': '22be219a8852c100a81f5d1e508b307d469829f50eaac9d603af126d1df3e10a',
    'docs/architecture/comment-study-productization-001.md': '912b1842faa4873c0ccb80d0ff3089340f6d462180e156f189ea896e706bffd0',
    'docs/data-contracts/comment-study-http-001.md': '02be8daad3a6df0f8d2706ee1d313edff9c184367bee0286a64a47ecf40bd3f2',
    'docs/data-contracts/comment-study-productization-001.md': '410049e2d9847575c3c54048c7af1c80265cd74106f17421152cb2d31697d549',
    'docs/design/pages/comment-study-productization-001.md': '04304df5e914481f06a12442316352671678f72c181564278bccde6bdb6fa9a5',
    'docs/plans/active/comment-study-automation-002.md': '682c29f8cdee0162b0b8a1df4d39b9ae8a9882a36bb8f1e684f315c608c1f505',
    'docs/plans/active/comment-study-productization-001.md': '1c86c15922503aadf432568b9f6fb4ca5401123e662b97546ad6c0bca3acaba7',
    'docs/runbooks/comment-study-productization-001.md': '4aebe8e5b2797c8399cfc48c85dda1539aa6d0bac49e89949c8fe9d280bc8946',
}
CONTRACT_DIR = Path('docs/data-contracts/comment-study-productization-001')


def closed_objects(value, location='$'):
    errors = []
    if isinstance(value, dict):
        types = value.get('type')
        if types == 'object' or isinstance(types, list) and 'object' in types:
            if value.get('additionalProperties') is not False:
                errors.append(location + ': object is not closed')
            if set(value.get('properties', {})) != set(value.get('required', [])):
                errors.append(location + ': required/properties disagree')
        for key, child in value.items():
            errors.extend(closed_objects(child, location + '.' + key))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            errors.extend(closed_objects(child, location + '[' + str(index) + ']'))
    return errors


def check(root):
    errors = []
    checks = []
    for name, expected in MANUAL_SHA256.items():
        path = root / name
        if not path.is_file():
            errors.append('missing manual: ' + name)
            continue
        body = path.read_bytes()
        matches = hashlib.sha256(body).hexdigest() == expected
        checks.append({'path': name, 'matchesApprovedV1': matches})
        if not matches:
            errors.append('approved v1 differs: ' + name + '; record an explicit version decision')
        for link in re.findall(r'\]\(([^)]+)\)', body.decode('utf-8')):
            target = link.split('#', 1)[0]
            if not target or '://' in target or target.startswith(('mailto:', '/')):
                continue
            if not (path.parent / target).resolve().is_file():
                errors.append('broken link: ' + name + ' -> ' + link)
    sizes = {}
    for stage in ('semantic', 'resolution', 'pair'):
        path = root / CONTRACT_DIR / (stage + '-output.schema.json')
        try:
            schema = json.loads(path.read_text())
            sizes[stage] = len(json.dumps(schema, ensure_ascii=False, separators=(',', ':')).encode())
            errors.extend(stage + ': ' + error for error in closed_objects(schema))
            if sizes[stage] > 6144:
                errors.append(stage + ': exceeds schema byte limit')
        except (OSError, ValueError) as error:
            errors.append(str(error))
    try:
        schema = json.loads((root / CONTRACT_DIR / 'start-run.schema.json').read_text())
        sample = json.loads((root / CONTRACT_DIR / 'start-run.example.json').read_text())
        if set(sample) != set(schema['required']):
            errors.append('start example does not have exactly the required root keys')
        if 'origin' in sample or sample.get('mode') != 'new_only' or sample.get('reason') is not None:
            errors.append('start example is not the approved manual new-only example')
    except (OSError, ValueError, KeyError) as error:
        errors.append(str(error))
    runbook = root / 'docs/runbooks/comment-study-productization-001.md'
    if runbook.is_file():
        ids = re.findall(r'^\| (T\d{2}) \|', runbook.read_text(), re.MULTILINE)
        if ids != ['T%02d' % number for number in range(1, 55)]:
            errors.append('acceptance IDs are not exactly T01 through T54')
    return {'level': 'document-static-only', 'manuals': checks,
            'transportSchemaBytes': sizes, 'errors': errors,
            'businessAcceptance': 'NOT_RUN', 'passed': not errors}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    result = check(args.root.resolve())
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if result['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
