#!/usr/bin/env node
// One filesystem pass replaces thousands of grep/sed processes. Keep the full
// document/link audit: deleting a target can break an otherwise unchanged file.
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const fields = ['状态', '最后核对', '适用范围', '事实来源', '冲突时以谁为准'];
const statuses = new Set(['权威当前', '代码事实优先', '活跃计划', '已完成计划', '一次性报告', '历史归档', '草案']);

export function headerErrors(path, text) {
  const values = new Map();
  let fence = null;
  for (const line of text.split(/\r?\n/)) {
    const marker = line.match(/^\s{0,3}(`{3,}|~{3,})/);
    if (marker) {
      if (!fence) fence = marker[1];
      else if (marker[1][0] === fence[0] && marker[1].length >= fence.length) fence = null;
      continue;
    }
    if (fence) continue;
    if (/^#{2,6}\s/.test(line)) break;
    const field = line.match(/^>\s*(状态|最后核对|适用范围|事实来源|冲突时以谁为准):\s*(.*?)\s*$/);
    if (field) values.set(field[1], [...(values.get(field[1]) ?? []), field[2]]);
  }
  const errors = [];
  for (const field of fields) {
    const entries = values.get(field) ?? [];
    if (entries.length !== 1 || !entries[0]) errors.push(`${path}: status header requires exactly one nonempty '${field}:' before the first section heading`);
  }
  const status = values.get('状态')?.[0];
  if (!statuses.has(status)) errors.push(`${path}: invalid document status '${status ?? ''}'`);
  if (path.startsWith('docs/plans/active/') && status !== '活跃计划') errors.push(`${path}: active plan must use status 活跃计划`);
  if (path.startsWith('docs/plans/completed/') && status !== '已完成计划') errors.push(`${path}: completed plan must use status 已完成计划`);
  return errors;
}

function markdownFiles(root, relative) {
  if (!existsSync(join(root, relative))) return [];
  return readdirSync(join(root, relative), { withFileTypes: true }).flatMap(entry => {
    const path = `${relative}/${entry.name}`;
    return entry.isDirectory() ? markdownFiles(root, path) : entry.isFile() && path.endsWith('.md') ? [path] : [];
  });
}

export function auditDocuments(root) {
  const errors = [];
  const docs = markdownFiles(root, 'docs');
  const index = readFileSync(join(root, 'docs/README.md'), 'utf8');
  const cache = new Map();
  const read = path => {
    if (!cache.has(path)) cache.set(path, readFileSync(join(root, path), 'utf8'));
    return cache.get(path);
  };
  for (const path of docs) {
    if (path !== 'docs/README.md' && !index.includes(`](${path.slice(5)})`)) errors.push(`${path}: document is not indexed in docs/README.md`);
    errors.push(...headerErrors(path, read(path)));
    if (/(最新版|最终版|final-final|copy|temp|draft[0-9]+)/.test(basename(path))) errors.push(`${path}: unstable lifecycle name`);
  }
  const links = ['README.md', 'AGENTS.md', 'CLAUDE.md', 'codx.md', 'references/README.md', ...docs, ...markdownFiles(root, 'database')];
  for (const path of links) {
    if (!existsSync(join(root, path))) continue;
    // Preserve the repository's inline-link coverage, including references in
    // examples. External URLs and root-relative application routes are exempt.
    for (const match of read(path).matchAll(/\]\(([^)]+)\)/g)) {
      const target = match[1].split('#')[0];
      if (!target || /^(https?:\/\/|mailto:|\/)/.test(target)) continue;
      if (!existsSync(resolve(root, dirname(path), target))) errors.push(`${path}: broken local link: ${target}`);
    }
  }
  return errors;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const root = fileURLToPath(new URL('../', import.meta.url));
  const errors = auditDocuments(root);
  for (const error of errors) console.error(`governance error: ${error}`);
  if (errors.length) process.exitCode = 1;
  else console.log('Document status, index and links passed');
}
