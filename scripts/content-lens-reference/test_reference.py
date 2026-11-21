"""Run from any directory with python3 -B path/to/test_reference.py."""
from collections import Counter
from copy import deepcopy
import json
import unittest

from lens_stats import account_residuals, agreement, rate, variance
from reference import ROOT, analysis_rows, answers, title_key


class ContentLensReference(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixture = json.loads((ROOT/'synthetic.json').read_text())
        cls.batch = json.loads((ROOT/'batch-cases.json').read_text())
        cls.result = answers(cls.fixture)

    def compare_answer(self, computed, golden):
        if isinstance(golden, dict):
            self.assertEqual(golden.keys(), computed.keys())
            for key, expected in golden.items():
                with self.subTest(key=key):
                    self.compare_answer(computed[key], expected)
        elif isinstance(golden, float):
            self.assertAlmostEqual(computed, golden, delta=1e-12)
        else:
            self.assertEqual(computed, golden)

    def test_frozen_answers(self):
        self.compare_answer(self.result, json.loads((ROOT/'answers.json').read_text()))

    def test_shared_example_and_snapshot_members(self):
        snapshots = self.result['sharedSnapshots']
        initial, filled, cleared, adjudicated = [snapshots[k] for k in ('initial','filled','cleared','adjudicated')]
        for snapshot in snapshots.values():
            self.assertEqual(snapshot['structureMembers'], ['p01', 'p02'])
            self.assertEqual(snapshot['titleKeys'], [title_key('样例A')])
            self.assertEqual(snapshot['creationLinks'], {'p01':'work-1','p02':'work-2'})
        self.assertEqual(initial['readings'], {'p01':[2,1200], 'p02':[None,None]})
        self.assertEqual(initial['midRate']['reason'], 'below_min_n')
        self.assertEqual(initial['readingDenominator'], ['p01'])
        self.assertEqual(filled['overThousand'], ['p01', 'p02'])
        self.assertEqual(cleared['readings']['p02'], [2,None])
        self.assertEqual(cleared['overThousand'], ['p01'])
        self.assertEqual(cleared['offers'], {'mechanism_explainer':2})
        self.assertEqual(adjudicated['offers'], {'actionable_method':2})

    def test_denominators_thresholds_and_wilson(self):
        rows = self.result['boundaryEffects']
        self.assertEqual(rows['19']['midRate']['reason'], 'below_min_n')
        self.assertEqual(rows['20']['midRate']['value'], 8/20)
        self.assertAlmostEqual(rows['20']['midRate']['low'], 0.21880396741419272)
        self.assertEqual(rows['21']['midRate']['value'], 8/21)
        self.assertEqual(rows['21']['activeCommentRate']['denominator'], 16)
        self.assertEqual(rows['21']['commentsPerThousandViews']['publications'], 15)
        self.assertEqual(self.result['zeroTotalViews']['reason'], 'zero_total_views')
        self.assertEqual(self.result['emptyGroupEffect']['midRate']['reason'], 'no_data')
        self.assertEqual(rate(20, 20, self.fixture['metric'])['high'], 1)
        self.assertEqual(rate(0, 20, self.fixture['metric'])['low'], 0)

    def test_agreement_exclusions_and_gate(self):
        cases = self.result['agreement']
        self.assertEqual(cases['meetsThreshold']['sampleCount'], 2)
        self.assertTrue(cases['meetsThreshold']['meetsThreshold'])
        self.assertEqual(cases['belowThreshold']['kappa'], 0.5)
        self.assertFalse(cases['belowThreshold']['meetsThreshold'])
        self.assertEqual(self.result['mixedRunAgreement'], dict(dimension='offer', structurePublications=22,
                         effect=None, reason='agreement_below_threshold'))
        self.assertIsNone(cases['chanceIsOne']['kappa'])
        self.assertEqual(cases['onlyExamples']['reason'], 'no_data')
        # Unequal marginals prevent a formula that assumes equal rater prevalence.
        paired = [[str(i), a, b, False] for i,(a,b) in enumerate(zip('xxxyy','xxyyy'))]
        self.assertAlmostEqual(agreement(paired, self.fixture['metric'])['kappa'], 8/13)

    def test_coverage_excludes_specials_only_from_main_grid(self):
        self.assertEqual(self.result['structure']['offer']['urgency_warning'], {'publications':0, 'share':0.0})
        self.assertEqual(self.result['structure']['offer']['unclear']['publications'], 2)
        grid = self.result['coverage']
        self.assertEqual(grid['labeledPublications'], 22)
        self.assertEqual(grid['emptyCells'], 167)
        self.assertEqual(grid['sparseCells'], 12)
        self.assertEqual(grid['top10Publications'], 12)
        self.assertEqual(grid['top10Share'], 12/22)

    def test_within_account_and_null_variance(self):
        metric = self.fixture['metric']
        rows = analysis_rows(self.fixture)
        residuals = account_residuals(rows, metric['min_account_posts'])
        self.assertEqual(len(residuals), 17)
        self.assertNotIn('p19', residuals)
        # At exactly five readings, a known account is included.
        known = [r for r in rows if r['views'] is not None][:5]
        self.assertEqual(len(account_residuals(known, 5)), 5)
        self.assertAlmostEqual(sum(account_residuals(known, 5).values()), 0)
        self.assertEqual(account_residuals([dict(r, account=None) for r in known], 5), {})
        self.assertEqual(self.result['zeroVariance']['reason'], 'zero_variance')
        self.assertEqual(variance([], 'offer', metric)['reason'], 'no_data')
        self.assertLess(self.result['variance']['offer']['excess'], 0)
        # With two labels perfectly determining two distinct readings, swapping
        # labels leaves eta²=1 on every draw; all ties count towards p.
        paired = [dict(publication='a', views=0, offer='x'), dict(publication='b', views=1, offer='y')]
        v = variance(paired, 'offer', metric)
        self.assertEqual((v['observed'], v['nullMean'], v['pValue']), (1,1,1))

    def test_order_invariance(self):
        reordered = deepcopy(self.fixture)
        for table in ['reports', 'publications', 'labels']:
            reordered[table].reverse()
        self.assertEqual(answers(reordered), self.result)

    def test_batch_vectors(self):
        keys = self.batch['titleKeys']
        self.assertEqual(keys, [title_key('样例 A!'), title_key('样例3')])
        offers = {r['offer'] for r in analysis_rows(self.fixture)}
        shard = self.batch['shards'][1]
        for case in self.batch['secondShardOutputs']:
            labels = [dict(self.batch['validLabel'], **row) for row in case['rows']]
            valid = sorted(r['keyIndex'] for r in labels) == shard['inputIndices']
            valid = valid and all(r['offer'] in offers for r in labels)
            self.assertEqual(valid, case['accepted'], case['case'])
        primary, repeat, audit = self.batch['submissions']
        self.assertEqual(primary, repeat)
        self.assertEqual(primary['titles'], audit['titles'])
        self.assertNotEqual(primary['expectedBatch'], audit['expectedBatch'])
        self.assertEqual(audit['parent'], primary['expectedBatch'])
        for step in self.batch['progress']:
            counts = Counter(step['shards'])
            self.assertEqual(step['total'], len(self.batch['shards']))
            self.assertEqual(step['completed'], counts['completed'])
            self.assertEqual(step['failed'], counts['failed'])


if __name__ == '__main__':
    unittest.main()
