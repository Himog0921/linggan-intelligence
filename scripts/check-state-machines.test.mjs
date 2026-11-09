import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { compareEntries, databaseStates, enumVariants, literalUsage, parseDoc, runCheck } from './check-state-machines.mjs';

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

test('a state-machine constraint replaces the earlier IN list', () => {
  const files = [
    { name: '0019.sql', text: "CREATE TABLE t (state text NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','in_progress','completed')));" },
    {
      name: '0097.sql',
      text: `ALTER TABLE t DROP CONSTRAINT IF EXISTS t_state_check;
        ALTER TABLE t ADD CONSTRAINT t_state_check CHECK (
          (state = 'pending' AND claimed_at IS NULL)
          OR (state = 'in_progress' AND claimed_at IS NOT NULL)
          OR (state = 'completed' AND completed_at IS NOT NULL)
          OR (state = 'input_blocked' AND claimed_at IS NULL)
        );`,
    },
  ];
  const found = databaseStates(files, new Map([['t', new Set(['state'])]]));
  const last = found.get('t.state').filter(e => e.kind === 'definition').pop();
  assert.deepEqual(last.values, ['completed', 'in_progress', 'input_blocked', 'pending']);
});

test('a constraint dropped with no readable replacement cannot be verified', () => {
  const key = 't.state';
  const entries = [
    { kind: 'definition', values: ['a', 'b'], source: { name: '0001.sql', index: 0 }, order: 0 },
    { kind: 'drop', source: { name: '0002.sql', index: 1 }, order: 1 },
  ];
  assert.match(compareEntries(key, ['a', 'b'], entries), /no readable replacement/);
});

test('a drop followed by a readable replacement keeps the documented set', () => {
  const entries = [
    { kind: 'definition', values: ['a', 'b'], source: { name: '0001.sql', index: 0 }, order: 0 },
    { kind: 'drop', source: { name: '0002.sql', index: 1 }, order: 1 },
    { kind: 'definition', values: ['a', 'b'], source: { name: '0002.sql', index: 1 }, order: 2 },
  ];
  assert.equal(compareEntries('t.state', ['a', 'b'], entries), null);
});

test('quotes in comments and char literals do not hide later statements', () => {
  const files = [{
    path: '/repo/crates/a.rs',
    text: "// a stray \" in a comment\nconst Q: char = '\"';\n" +
      'sqlx::query("UPDATE collection_run SET phase=\'ghost\' WHERE run_ref=$1");',
  }];
  const errors = literalUsage(files, new Map([['collection_run', new Map([['phase', new Set(['discovery'])]])]]));
  assert.equal(errors.length, 1);
  assert.match(errors[0], /collection_run\.phase='ghost'/);
});

test('= ANY (ARRAY[...]) counts as a domain and as a replacement', () => {
  const files = [
    { name: '0001.sql', text: "CREATE TABLE t (state text CHECK (state IN ('a','b')));" },
    { name: '0002.sql', text: "ALTER TABLE t DROP CONSTRAINT t_state_check; ALTER TABLE t ADD CONSTRAINT t_state_check CHECK (state = ANY (ARRAY['b'::text,'c'::text]));" },
  ];
  const found = databaseStates(files, new Map([['t', new Set(['state'])]]));
  assert.deepEqual(compareEntries('t.state', ['b', 'c'], found.get('t.state')), null);
});

test('a single-value narrowing migration wins', () => {
  const files = [
    { name: '0001.sql', text: "CREATE TABLE t (state text CHECK (state IN ('a','b')));" },
    { name: '0002.sql', text: "ALTER TABLE t DROP CONSTRAINT t_state_check; ALTER TABLE t ADD CONSTRAINT t_state_check CHECK (state IN ('a'));" },
  ];
  const found = databaseStates(files, new Map([['t', new Set(['state'])]]));
  const last = found.get('t.state').filter(e => e.kind === 'definition').pop();
  assert.deepEqual(last.values, ['a']);
});

test('a neighbouring column list is not attributed to this column', () => {
  const files = [{ name: '0001.sql', text: "CREATE TABLE t (state text CHECK (state = 'b' AND mode IN ('x','y')), mode text);" }];
  const found = databaseStates(files, new Map([['t', new Set(['state'])]]));
  assert.equal(found.get('t.state'), undefined);
});

test('a semantic constraint name is recognised when dropped', () => {
  const files = [{ name: '0001.sql', text: "ALTER TABLE t DROP CONSTRAINT reading_current_publication;" }];
  const found = databaseStates(files, new Map([['t', new Set(['publication'])]]));
  assert.equal(found.get('t.publication')[0].kind, 'drop');
});

test('a drop performed by a PL/pgSQL loop counts as a drop', () => {
  const files = [{
    name: '0001.sql',
    text: `DO $$ BEGIN
      FOR c IN SELECT oid FROM pg_constraint WHERE pg_get_constraintdef(oid) LIKE '%stop_reason%' LOOP
        EXECUTE format('ALTER TABLE t DROP CONSTRAINT %I', c);
      END LOOP; END $$;`,
  }];
  const found = databaseStates(files, new Map([['t', new Set(['stop_reason'])]]));
  assert.equal(found.get('t.stop_reason')[0].kind, 'drop');
});

test('adding values without dropping the old constraint is rejected', () => {
  const entries = [
    { kind: 'definition', values: ['a', 'b'], source: { name: '0001.sql', index: 0 }, order: 0 },
    { kind: 'definition', values: ['a', 'b', 'c'], source: { name: '0002.sql', index: 1 }, order: 1 },
  ];
  assert.match(compareEntries('t.state', ['a', 'b', 'c'], entries), /without dropping the earlier constraint/);
});

test('DROP and ADD in one statement is a replacement, not a bare drop', () => {
  const files = [{
    name: '0001.sql',
    text: "ALTER TABLE t DROP CONSTRAINT t_state_check, ADD CONSTRAINT t_state_check CHECK (state IN ('a','b'));",
  }];
  const found = databaseStates(files, new Map([['t', new Set(['state'])]]));
  assert.equal(compareEntries('t.state', ['a', 'b'], found.get('t.state')), null);
});

test('tuple and struct enum variants are all read', () => {
  const dir = mkdtempSync(join(tmpdir(), 'state-machines-'));
  const path = join(dir, 'x.rs');
  writeFileSync(path, 'pub enum E {\n    Alpha,\n    Blocked(i32),\n    Custom { reason: String },\n}\n');
  assert.deepEqual(enumVariants(path).get('E'), ['alpha', 'blocked', 'custom']);
});

test('the repository matches docs/architecture/state-machines.md', () => {
  const { errors } = runCheck();
  assert.deepEqual(errors, []);
});
