#!/usr/bin/env node
// The state vocabulary lives in two places that can silently drift: the CHECK
// constraints in database/migrations and the doc's tables. This compares them.
// It also reads state literals out of SQL that updates a documented table, so a
// new value cannot appear in code while the doc still claims the old set.
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = resolve(fileURLToPath(new URL('..', import.meta.url)));
const DOC = 'docs/architecture/state-machines.md';

const toArray = value => value.split(',').map(s => s.trim().replace(/`/g, '')).filter(Boolean).sort();
const eq = (a, b) => a.length === b.length && a.every((v, i) => v === b[i]);
const show = list => list.map(v => `'${v}'`).join(', ') || '(none)';

export function parseDoc(text) {
  const tables = [];
  const code = [];
  let header = null;
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line.startsWith('|')) { header = null; continue; }
    const cells = line.slice(1, line.endsWith('|') ? -1 : undefined).split('|').map(c => c.trim());
    if (cells.every(c => /^-+$/.test(c))) continue;
    const stripped = cells.map(c => c.replace(/`/g, '').trim());
    if (stripped[0] === '表' && stripped[1] === '列') { header = 'table'; continue; }
    if (stripped[0] === '代码' && stripped[1] === '枚举') { header = 'code'; continue; }
    if (!header || stripped.length < 3) continue;
    if (header === 'table') tables.push({ table: stripped[0], column: stripped[1], states: toArray(stripped[2]) });
    else code.push({ file: stripped[0], name: stripped[1], states: toArray(stripped[2]) });
  }
  return { tables, code };
}

function matchingParen(text, open) {
  let depth = 0;
  for (let i = open; i < text.length; i += 1) {
    if (text[i] === '(') depth += 1;
    else if (text[i] === ')') { depth -= 1; if (depth === 0) return i; }
  }
  return text.length;
}

// A column definition ends at the next column, or at a table-level clause.
const CLAUSE_OR_COLUMN = /\n\s{1,8}(?:[A-Z]{3,}\b|[a-z_][a-z0-9_]*\s+(?:text|uuid|timestamptz|timestamp|integer|int|bigint|boolean|jsonb|numeric|varchar|character|date)\b)/g;

function inLists(slice) {
  const values = new Set();
  for (const m of slice.matchAll(/(?<!NOT\s)IN\s*\(([^()]*)\)/gi)) {
    const literals = [...m[1].matchAll(/'([a-z0-9_-]+)'/g)].map(x => x[1]);
    if (literals.length >= 2) literals.forEach(v => values.add(v));
  }
  return [...values];
}

// Most constraints repeat the column before or inside the list: CHECK (col IN (...)).
// Requiring that avoids reading a different column's list as this column's domain.
// NOT IN is always a restriction, never a domain, so it is skipped outright.
function columnInLists(text, column) {
  const values = new Set();
  const marker = new RegExp(`\\b${column}\\b`);
  for (const m of text.matchAll(/(?<!NOT\s)IN\s*\(([^()]*)\)/gi)) {
    const before = text.slice(Math.max(0, m.index - 80), m.index);
    if (!marker.test(before) && !marker.test(m[1])) continue;
    const literals = [...m[1].matchAll(/'([a-z0-9_-]+)'/g)].map(x => x[1]);
    if (literals.length >= 2) literals.forEach(v => values.add(v));
  }
  return [...values];
}

function columnSlice(text, column) {
  const finder = new RegExp(`\\b${column}\\s+(?:text|varchar|character)\\b`, 'g');
  const found = finder.exec(text);
  if (!found) return null;
  CLAUSE_OR_COLUMN.lastIndex = found.index + found[0].length;
  const next = CLAUSE_OR_COLUMN.exec(text);
  return text.slice(found.index, next ? next.index : text.length);
}

function tableBodies(text) {
  const bodies = new Map();
  for (const m of text.matchAll(/CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(?:public\.)?([a-z0-9_]+)\s*\(/gi)) {
    const open = m.index + m[0].length - 1;
    bodies.set(m[1].toLowerCase(), text.slice(open + 1, matchingParen(text, open)));
  }
  return bodies;
}

export function databaseStates(files, wanted) {
  const found = new Map();
  const remember = (key, values, source) => {
    if (!values.length) return;
    if (!found.has(key)) found.set(key, []);
    found.get(key).push({ values: [...values].sort(), source });
  };
  for (const [index, { name, text }] of files.entries()) {
    const bodies = tableBodies(text);
    for (const [table, columns] of wanted) {
      const body = bodies.get(table);
      if (!body) continue;
      for (const column of columns) {
        const direct = columnInLists(body, column);
        if (direct.length) { remember(`${table}.${column}`, direct, { name, index }); continue; }
        const slice = columnSlice(body, column);
        if (slice) remember(`${table}.${column}`, inLists(slice), { name, index });
      }
    }
    for (const m of text.matchAll(/ALTER\s+TABLE\s+(?:IF\s+EXISTS\s+)?(?:public\.)?([a-z0-9_]+)\b[\s\S]*?;/gi)) {
      const table = m[1].toLowerCase();
      const statement = m[0];
      const columns = wanted.get(table);
      if (!columns) continue;
      for (const column of columns) {
        const direct = columnInLists(statement, column);
        if (direct.length) { remember(`${table}.${column}`, direct, { name, index }); continue; }
        if (!new RegExp(`\\b${column}\\b`).test(statement)) continue;
        const finder = new RegExp(`\\b${column}\\b`, 'g');
        for (const hit of statement.matchAll(finder)) {
          const rest = statement.slice(hit.index);
          const end = rest.slice(1).search(/\bADD\s+(?:COLUMN|CONSTRAINT)\b/i);
          remember(`${table}.${column}`, inLists(end < 0 ? rest : rest.slice(0, end + 1)), { name, index });
        }
      }
    }
  }
  return found;
}

export function rustFiles(dir) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'tests' || entry.name === 'fixtures' || entry.name === 'target') return [];
      return rustFiles(path);
    }
    if (!entry.name.endsWith('.rs')) return [];
    if (/_tests?\.rs$/.test(entry.name)) return [];
    return [path];
  });
}

function stringLiterals(text) {
  const out = [];
  for (const m of text.matchAll(/r(#*)"([\s\S]*?)"\1/g)) out.push(m[2]);
  for (const m of text.matchAll(/"((?:[^"\\]|\\.)*)"/g)) out.push(m[1]);
  return out;
}

export function literalUsage(files, wanted) {
  const errors = [];
  for (const { path, text } of files) {
    const relative = path.slice(root.length + 1);
    for (const literal of stringLiterals(text)) {
      for (const statement of literal.split(';')) {
        const target = statement.match(/\b(?:UPDATE|INSERT\s+INTO|DELETE\s+FROM)\s+(?:public\.)?([a-z0-9_]+)/i);
        if (!target) continue;
        const columns = wanted.get(target[1].toLowerCase());
        if (!columns) continue;
        for (const [column, allowed] of columns) {
          const pattern = new RegExp(`\\b${column}\\s*(?:=|<>|!=)\\s*'([A-Za-z0-9_-]+)'`, 'g');
          for (const m of statement.matchAll(pattern)) {
            if (!allowed.has(m[1])) errors.push(`${relative}: ${target[1]}.${column}='${m[1]}' is not in ${DOC}`);
          }
        }
      }
    }
  }
  return errors;
}

export function enumVariants(path) {
  if (!existsSync(path)) return null;
  const text = readFileSync(path, 'utf8');
  const out = new Map();
  for (const m of text.matchAll(/pub\s+enum\s+([A-Za-z0-9_]+)\s*\{/g)) {
    const open = m.index + m[0].length - 1;
    const body = text.slice(open + 1, matchingParen(text, open));
    out.set(m[1], [...body.matchAll(/^\s*([A-Z][A-Za-z0-9_]*)\s*[,{(]?\s*$/gm)]
      .map(x => x[1].replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase())
      .sort());
  }
  return out;
}

export function runCheck(repoRoot = root) {
  const doc = readFileSync(join(repoRoot, DOC), 'utf8');
  const { tables, code } = parseDoc(doc);
  const errors = [];

  const wanted = new Map();
  for (const entry of tables) {
    if (!wanted.has(entry.table)) wanted.set(entry.table, new Set());
    wanted.get(entry.table).add(entry.column);
  }

  const migrations = readdirSync(join(repoRoot, 'database/migrations'))
    .filter(f => f.endsWith('.sql')).sort()
    .map(name => ({ name, text: readFileSync(join(repoRoot, 'database/migrations', name), 'utf8') }));
  const db = databaseStates(migrations, wanted);

  for (const entry of tables) {
    const key = `${entry.table}.${entry.column}`;
    const definitions = db.get(key);
    if (!definitions) { errors.push(`${key}: documented, but no CHECK constraint found in database/migrations`); continue; }
    // Migrations apply in filename order, and a narrowing migration drops the old
    // constraint and adds a full new one, so the last file wins. Within one file a
    // second, narrower CHECK on the same column is an extra condition, not a new
    // domain: keep the widest of that file. Incomparable sets mean a real conflict.
    const byFile = new Map();
    for (const definition of definitions) {
      if (!byFile.has(definition.source.index)) byFile.set(definition.source.index, []);
      byFile.get(definition.source.index).push(definition);
    }
    const latest = [...byFile.keys()].sort((a, b) => a - b).map(index => byFile.get(index)).pop();
    const widest = latest.reduce((a, b) => (a.values.every(v => b.values.includes(v)) ? b : a));
    const incomparable = latest.filter(s => !s.values.every(v => widest.values.includes(v)));
    if (incomparable.length) {
      const detail = latest.map(d => `${show(d.values)} (${d.source.name})`).join(' vs ');
      errors.push(`${key}: database defines incompatible sets: ${detail}`);
      continue;
    }
    if (!eq(widest.values, entry.states)) {
      const extra = widest.values.filter(v => !entry.states.includes(v));
      const missing = entry.states.filter(v => !widest.values.includes(v));
      errors.push(`${key}: doc ${show(entry.states)} != database ${show(widest.values)} (${widest.source.name})` +
        `${extra.length ? ` | database-only: ${show(extra)}` : ''}${missing.length ? ` | doc-only: ${show(missing)}` : ''}`);
    }
  }

  for (const entry of code) {
    const variants = enumVariants(join(repoRoot, entry.file));
    if (!variants) { errors.push(`${entry.file}: documented enum source does not exist`); continue; }
    const actual = variants.get(entry.name);
    if (!actual) { errors.push(`${entry.file}: enum ${entry.name} not found`); continue; }
    if (!eq(actual, entry.states)) {
      const extra = actual.filter(v => !entry.states.includes(v));
      const missing = entry.states.filter(v => !actual.includes(v));
      errors.push(`${entry.name}: doc ${show(entry.states)} != code ${show(actual)}` +
        `${extra.length ? ` | code-only: ${show(extra)}` : ''}${missing.length ? ` | doc-only: ${show(missing)}` : ''}`);
    }
  }

  const allowedByTable = new Map();
  for (const entry of tables) {
    if (!allowedByTable.has(entry.table)) allowedByTable.set(entry.table, new Map());
    const columns = allowedByTable.get(entry.table);
    const allowed = columns.get(entry.column) ?? new Set();
    entry.states.forEach(v => allowed.add(v));
    columns.set(entry.column, allowed);
  }
  const sources = rustFiles(join(repoRoot, 'crates')).concat(rustFiles(join(repoRoot, 'apps')))
    .map(path => ({ path, text: readFileSync(path, 'utf8') }));
  errors.push(...literalUsage(sources, allowedByTable));

  return { errors, tables, code };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const result = runCheck();
  if (process.argv.includes('--list')) {
    for (const e of result.tables) console.log(`${e.table}.${e.column}: ${e.states.join(', ')}`);
    for (const e of result.code) console.log(`${e.name}: ${e.states.join(', ')}`);
    process.exit(0);
  }
  if (result.errors.length) {
    for (const error of result.errors) console.error(`state-machines: ${error}`);
    console.error(`state-machines: ${result.errors.length} problem(s) against ${DOC}`);
    process.exitCode = 1;
  } else {
    console.log(`state-machines: ${result.tables.length} columns + ${result.code.length} enums match ${DOC}`);
  }
}
