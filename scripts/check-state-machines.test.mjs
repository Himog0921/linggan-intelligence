import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { databaseStates, enumVariants, literalUsage, parseDoc, runCheck } from './check-state-machines.mjs';

const doc = `
| 表 | 列 | 允许状态 |
| --- | --- | --- |
| collection_run | control_intent | \`running\`, \`paused\`, \`stopped\` |

| 从 | 到 | 条件 | 写入点 |
| --- | --- | --- | --- |
| running | paused | 用户点暂停 | a.rs:1 |

| 代码 | 枚举 | 允许状态 |
| --- | --- | --- |
| crates/x.rs | ColumnState | \`available\`, \`waiting\` |
`;

test('parseDoc reads state tables and ignores the transitions table', () => {
  const parsed = parseDoc(doc);
  assert.deepEqual(parsed.tables, [{ table: 'collection_run', column: 'control_intent', states: ['paused', 'running', 'stopped'] }]);
  assert.deepEqual(parsed.code, [{ file: 'crates/x.rs', name: 'ColumnState', states: ['available', 'waiting'] }]);
});

test('a narrower CHECK in the same migration is a condition, not a second domain', () => {
  const files = [{
    name: '0036.sql',
    text: `ALTER TABLE collection_work_order
      ADD COLUMN queue_state text NOT NULL DEFAULT 'legacy'
        CHECK (queue_state IN ('legacy','queued','leased','completed','cancelled')),
      ADD CONSTRAINT scheduled CHECK (queue_state IN ('legacy','cancelled') OR scheduled_for IS NOT NULL);`,
  }];
  const found = databaseStates(files, new Map([['collection_work_order', new Set(['queue_state'])]]));
  assert.deepEqual(found.get('collection_work_order.queue_state')[0].values, ['cancelled', 'completed', 'leased', 'legacy', 'queued']);
});

test('a later migration that narrows a domain wins', () => {
  const files = [
    { name: '0146.sql', text: "CREATE TABLE t (publication text NOT NULL CHECK (publication IN ('automatic','candidate','human_edit')));" },
    { name: '0168.sql', text: "ALTER TABLE t ADD CONSTRAINT p CHECK(publication IN('automatic','human_edit'));" },
  ];
  const found = databaseStates(files, new Map([['t', new Set(['publication'])]]));
  assert.deepEqual(found.get('t.publication').at(-1).values, ['automatic', 'human_edit']);
});

test('NOT IN never counts as the domain', () => {
  const files = [
    { name: '0005.sql', text: "CREATE TABLE t (lifecycle_state text NOT NULL CHECK (lifecycle_state IN ('a','b','c')));" },
    { name: '0042.sql', text: "ALTER TABLE t ADD CONSTRAINT k CHECK (kind <> 'k' OR lifecycle_state NOT IN ('a','b'));" },
  ];
  const found = databaseStates(files, new Map([['t', new Set(['lifecycle_state'])]]));
  assert.deepEqual(found.get('t.lifecycle_state')[0].values, ['a', 'b', 'c']);
});

test('literals are attributed to the table the statement updates', () => {
  const files = [{
    path: '/repo/crates/a.rs',
    text: 'sqlx::query("UPDATE collection_run SET phase=\'ghost\' WHERE run_ref=$1");\n' +
      'sqlx::query("UPDATE other_table SET state=\'ghost\' WHERE x=$1");',
  }];
  const wanted = new Map([['collection_run', new Map([['phase', new Set(['discovery', 'ended'])]])]]);
  const errors = literalUsage(files, wanted);
  assert.equal(errors.length, 1);
  assert.match(errors[0], /collection_run\.phase='ghost'/);
});

test('enum variants are compared in snake_case', () => {
  const dir = mkdtempSync(join(tmpdir(), 'state-machines-'));
  const path = join(dir, 'x.rs');
  writeFileSync(path, 'pub enum ColumnState {\n    Available,\n    NotProvided,\n}\n');
  assert.deepEqual(enumVariants(path).get('ColumnState'), ['available', 'not_provided']);
});

test('the repository matches docs/architecture/state-machines.md', () => {
  const { errors } = runCheck();
  assert.deepEqual(errors, []);
});
