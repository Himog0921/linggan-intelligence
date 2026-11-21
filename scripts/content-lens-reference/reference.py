"""Run with python3 -B reference.py to print the reproducible synthetic answers."""
from collections import Counter
from hashlib import md5
import json
from math import fsum
from pathlib import Path
from string import punctuation

from lens_stats import Shuffle, account_residuals, agreement, coverage, effect, variance

ROOT = Path(__file__).resolve().parent


def title_key(title):
    # The synthetic titles exercise ASCII whitespace/punctuation and Chinese letters.
    normalized = ''.join(c for c in title.lower() if not c.isspace() and c not in punctuation)
    return md5(normalized.encode('utf-8')).hexdigest()


def table(fixture, name, fields):
    return [dict(zip(fixture[fields], row, strict=True)) for row in fixture[name]]


def analysis_rows(fixture, extra_reports=(), decisions=()):
    labels = {title_key(r['title']): r for r in table(fixture, 'labels', 'labelFields')}
    latest = {}
    for publication, sequence, value in [*fixture['reports'], *extra_reports]:
        if publication not in latest or latest[publication][0] < sequence:
            latest[publication] = (sequence, value)
    rows = []
    for record in table(fixture, 'publications', 'publicationFields'):
        key = title_key(record['title'])
        sequence, views = latest.get(record['publication'], (None, None))
        label = dict(labels[key])
        for title, dimension, new_value in decisions:
            if key == title_key(title):
                label[dimension] = new_value
        row = {**record, **label, 'titleKey': key, 'views': views, 'reportSequence': sequence}
        del row['title']
        rows.append(row)
    return sorted(rows, key=lambda row: row['publication'])


def shared_snapshot(rows, metric):
    return dict(titleKeys=sorted({r['titleKey'] for r in rows}),
                structureMembers=[r['publication'] for r in rows],
                offers=dict(Counter(r['offer'] for r in rows)),
                readingDenominator=[r['publication'] for r in rows if r['views'] is not None],
                overThousand=[r['publication'] for r in rows if r['views'] is not None
                              and r['views'] >= metric['views_threshold_mid']],
                creationLinks={r['publication']: r['work'] for r in rows},
                readings={r['publication']: [r['reportSequence'], r['views']] for r in rows},
                midRate=effect(rows, metric)['midRate'])


def answers(fixture):
    rows = analysis_rows(fixture)
    metric = fixture['metric']
    # All four snapshots have the same publication scope and label method.
    snapshots = {}
    for state in fixture['sameScopeSnapshots']:
        updated = analysis_rows(fixture, state['extraReports'], state['decisions'])
        snapshots[state['id']] = shared_snapshot(updated[:2], metric)

    boundaries = {}
    for n in [19, 20, 21]:
        # The second publication has no report, so n+1 publications have n known readings.
        boundaries[str(n)] = effect(rows[:n+1], metric)
    residual = account_residuals(rows, metric['min_account_posts'])
    structure = {}
    for dimension, keys in fixture['dimensionKeys'].items():
        counts = Counter(r[dimension] for r in rows)
        structure[dimension] = {key: {'publications': counts[key], 'share': counts[key]/len(rows)}
                                for key in keys}

    by_dimension = {}
    for dimension in ['moment', 'offer']:
        by_dimension[dimension] = {}
        for label in sorted({r[dimension] for r in rows}):
            group = [r for r in rows if r[dimension] == label]
            residuals = [residual[r['publication']] for r in group if r['publication'] in residual]
            by_dimension[dimension][label] = {
                **effect(group, metric),
                'withinAccountResidual': dict(publications=len(residuals),
                    value=fsum(residuals)/len(residuals) if residuals else None)
            }
    # p03 has observed zero readings and zero comments.
    zero = [r for r in rows if r['views'] == 0]
    all_keys = sorted({r['titleKey'] for r in rows})
    agreement_cases = {name: agreement(pairs, metric) for name, pairs in fixture['agreementCases'].items()}
    # Two primary runs contribute to the same dimension. Every run must pass C4.
    offer_gates = [agreement_cases[k] for k in ('meetsThreshold', 'belowThreshold')]
    passes = all(check['meetsThreshold'] for check in offer_gates)
    mixed = dict(dimension='offer', structurePublications=sum(r['publications'] for r in structure['offer'].values()),
                 effect=by_dimension['offer'] if passes else None,
                 reason=None if passes else 'agreement_below_threshold')
    return dict(sharedSnapshots=snapshots, boundaryEffects=boundaries, structure=structure,
                coverage=coverage(rows, fixture['dimensionKeys']), effects=by_dimension,
                quality=dict(publications=len(rows), titleKeys=len(all_keys), withViews=sum(r['views'] is not None for r in rows),
                             latestPublishedDate=max(r['published'] for r in rows)),
                mixedRunAgreement=mixed,
                variance={factor:variance(rows, factor, metric) for factor in ['account','offer','moment']},
                withinAccountVariance={d:variance(rows, d, metric, True) for d in ['offer','moment']},
                agreement=agreement_cases,
                auditSample3=Shuffle(metric['audit_sample_seed']).shuffled(all_keys)[:3],
                zeroVariance=variance(zero, 'offer', metric),
                zeroTotalViews=effect(zero, metric)['commentsPerThousandViews'],
                emptyGroupEffect=effect([], metric))


if __name__ == '__main__':
    fixture = json.loads((ROOT/'synthetic.json').read_text())
    print(json.dumps(answers(fixture), ensure_ascii=False, indent=2, allow_nan=False))
