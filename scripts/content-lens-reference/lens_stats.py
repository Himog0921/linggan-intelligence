"""Independent numerical oracle for the synthetic content-lens fixtures (Python stdlib)."""
from collections import Counter, defaultdict
from math import fsum, log1p, sqrt
from statistics import median


class Shuffle:
    """Park–Miller 16807 + rejection sampling + descending Fisher–Yates."""

    def __init__(self, seed):
        self.state = seed
        if not 0 < seed < 2147483647:
            raise ValueError("Park–Miller seed must be between 1 and 2147483646")

    def shuffled(self, items):
        result = list(items)
        for end in range(len(result) - 1, 0, -1):
            size = end + 1
            while True:
                self.state = self.state * 16807 % 2147483647
                draw = self.state - 1
                if draw < 2147483646 - 2147483646 % size:
                    break
            index = draw % size
            result[index], result[end] = result[end], result[index]
        return result


def agreement(pairs, metric):
    pairs = [(a, b) for _, a, b, example in pairs if not example]
    n = len(pairs)
    if not n:
        return dict(sampleCount=0, agreement=None, kappa=None, reason='no_data', meetsThreshold=False)
    matches = sum(a == b for a, b in pairs)
    left, right = (Counter(side) for side in zip(*pairs))
    # Use counts until the final division. pe is the dot product divided by n².
    chance = sum(count * right[key] for key, count in left.items())
    kappa = None if chance == n*n else (matches*n-chance) / (n*n-chance)
    return dict(sampleCount=n, agreement=matches/n, kappa=kappa,
                reason='kappa_undefined' if kappa is None else None,
                meetsThreshold=kappa is not None and kappa >= metric['min_kappa']
                and matches/n >= metric['min_agreement'])


def rate(hits, n, metric):
    result = dict(numerator=hits, denominator=n, value=None, low=None, high=None)
    if n < metric['min_n_rate']:
        return dict(result, reason='below_min_n' if n else 'no_data')
    p = hits/n
    z = metric['wilson_z']
    base = 1 + z*z/n
    center = (p + z*z/(2*n)) / base
    margin = z * sqrt(p*(1-p)/n + z*z/(4*n*n)) / base
    return dict(result, value=p, low=0.0 if hits == 0 else center-margin,
                high=1.0 if hits == n else center+margin, reason=None)


def effect(rows, metric):
    views = [r['views'] for r in rows if r['views'] is not None]
    comments = [r['comments'] for r in rows if r['comments'] is not None]
    both = [r for r in rows if r['comments'] is not None and r['views'] is not None]
    sum_views = sum(r['views'] for r in both)
    reason = None if sum_views else ('zero_total_views' if both else 'no_data')
    return dict(publications=len(rows), withViews=len(views),
                medianViews=median(views) if views else None,
                midRate=rate(sum(v >= metric['views_threshold_mid'] for v in views), len(views), metric),
                highCount=sum(v >= metric['views_threshold_high'] for v in views),
                activeCommentRate=rate(sum(c >= metric['comments_threshold_active'] for c in comments),
                                       len(comments), metric),
                commentsPerThousandViews=dict(publications=len(both), reason=reason, value=
                    1000*sum(r['comments'] for r in both)/sum_views if sum_views else None))


def coverage(rows, dimension_keys):
    cells = Counter((r['moment'], r['offer']) for r in rows)
    mainstream = [n for (m,o), n in cells.items() if m not in ('other','unclear') and o not in ('other','unclear')]
    top = sum(sorted(cells.values(), reverse=True)[:10])
    main_rows = set(dimension_keys['moment']) - {'other', 'unclear'}
    main_columns = set(dimension_keys['offer']) - {'other', 'unclear'}
    return dict(emptyCells=len(main_rows)*len(main_columns)-len(mainstream), sparseCells=sum(n <= 2 for n in mainstream),
                top10Share=top/len(rows) if rows else None, top10Publications=top,
                labeledPublications=len(rows))


def account_residuals(rows, min_posts):
    accounts = defaultdict(list)
    for r in rows:
        if r['account'] is not None and r['views'] is not None:
            accounts[r['account']].append(log1p(r['views']))
    means = {k: fsum(vals)/len(vals) for k, vals in accounts.items() if len(vals) >= min_posts}
    return {r['publication']: log1p(r['views']) - means[r['account']] for r in rows
            if r['account'] in means and r['views'] is not None}


def variance(rows, factor, metric, within_account=False):
    rows = sorted((r for r in rows if r['views'] is not None), key=lambda r: r['publication'])
    if within_account:
        residuals = account_residuals(rows, metric['min_account_posts'])
        rows = [r for r in rows if r['publication'] in residuals]
        values = [residuals[r['publication']] for r in rows]
    else:
        values = [log1p(r['views']) for r in rows]

    empty = dict(observations=len(rows), observed=None, nullMean=None, excess=None, pValue=None)
    if not values:
        return dict(empty, reason='no_data')
    mean = fsum(values)/len(values)
    total = fsum((v-mean)**2 for v in values)
    if min(values) == max(values):
        return dict(empty, reason='zero_variance')

    def eta(labels):
        groups = defaultdict(list)
        for label, v in zip(labels, values):
            groups[label].append(v)
        return fsum(len(g)*(fsum(g)/len(g)-mean)**2 for g in groups.values()) / total

    labels = [r[factor] for r in rows]
    strata = defaultdict(list)
    for i, row in enumerate(rows):
        strata[row['account'] if within_account else 'all'].append(i)
    observed = eta(labels)
    shuffled_values = []
    random = Shuffle(metric['permutation_seed'])
    # Restart from original labels on each draw; visit strata in account key order.
    for _ in range(metric['permutation_count']):
        draw = list(labels)
        for key in sorted(strata):
            indices = strata[key]
            permuted = random.shuffled(labels[i] for i in indices)
            for i, label in zip(indices, permuted):
                draw[i] = label
        shuffled_values.append(eta(draw))
    null_mean = fsum(shuffled_values)/len(shuffled_values)
    return dict(observations=len(rows), observed=observed, nullMean=null_mean,
                excess=observed-null_mean, pValue=(1+sum(x >= observed for x in shuffled_values))
                / (1+len(shuffled_values)), reason=None)
