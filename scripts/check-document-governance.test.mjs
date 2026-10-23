import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { auditDocuments, headerErrors } from './check-document-governance.mjs';

const header = '> 状态: 权威当前\n> 最后核对: 2026-10-23\n> 适用范围: synthetic\n> 事实来源: synthetic\n> 冲突时以谁为准: synthetic\n';

test('metadata survives introduction growth beyond twelve lines and CRLF', () => {
  const text = '# Document\n' + '\nintro\n'.repeat(15) + header + '\n## Body\n';
  assert.deepEqual(headerErrors('docs/example.md', text), []);
  assert.deepEqual(headerErrors('docs/example.md', text.replaceAll('\n', '\r\n')), []);
});

test('missing, duplicate, empty and invalid metadata fail without accepting body examples', () => {
  for (const text of [
    header.replace('> 事实来源: synthetic\n', ''),
    header + '> 状态: 草案\n',
    header.replace('> 适用范围: synthetic', '> 适用范围: '),
    header.replace('权威当前', 'approved'),
    '## Example\n' + header,
    '```markdown\n' + header + '```\n',
    '~~~\n' + header + '~~~\n',
  ]) assert.ok(headerErrors('docs/example.md', text).length, text);
});

test('lifecycle status still matches active and completed plan directories', () => {
  assert.ok(headerErrors('docs/plans/active/example.md', header).length);
  assert.ok(headerErrors('docs/plans/completed/example.md', header).length);
  assert.deepEqual(headerErrors('docs/plans/active/example.md', header.replace('权威当前', '活跃计划')), []);
  assert.deepEqual(headerErrors('docs/plans/completed/example.md', header.replace('权威当前', '已完成计划')), []);
});

test('full scan catches unindexed documents and links broken by target deletion', () => {
  const root = mkdtempSync(join(tmpdir(), 'linggan-doc-governance-'));
  try {
    mkdirSync(join(root, 'docs'));
    writeFileSync(join(root, 'docs/README.md'), '# Index\n' + header + '\n## Docs\n[Example](example.md)\n');
    writeFileSync(join(root, 'docs/example.md'), '# Example\n' + header + '\n## Body\n[Target](../target.txt)\n');
    writeFileSync(join(root, 'target.txt'), 'synthetic');
    assert.deepEqual(auditDocuments(root), []);
    rmSync(join(root, 'target.txt'));
    assert.ok(auditDocuments(root).some(error => error.includes('broken local link')));
    writeFileSync(join(root, 'docs/new.md'), '# New\n' + header);
    assert.ok(auditDocuments(root).some(error => error.includes('not indexed')));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
