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

// Both spellings of a value domain occur in this schema: IN ('a','b') and
// = ANY (ARRAY['a'::text,'b'::text]). A one-element list counts: a migration may
// narrow a column to a single value, and ignoring that would keep the older set.
function literals(slice) {
  return [...slice.matchAll(/'([A-Za-z0-9_-]+)'/g)].map(x => x[1]);
}

const DOMAIN_LIST = /(?<!NOT\s)(?:IN\s*\(([^()]*)\)|=\s*ANY\s*\(\s*ARRAY\s*\[([^\]]*)\])/gi;

function inLists(slice) {
  const values = new Set();
  for (const m of slice.matchAll(DOMAIN_LIST)) literals(m[1] ?? m[2]).forEach(v => values.add(v));
  return [...values];
}

// Most constraints name the column immediately before the list: CHECK (col IN (...)).
// Requiring that avoids reading a neighbouring column's list as this column's domain,
// which happens whenever both appear in one constraint.
function columnInLists(text, column) {
  const values = new Set();
  const anchored = new RegExp(`\\b${column}$`);
  for (const m of text.matchAll(DOMAIN_LIST)) {
    const before = text.slice(Math.max(0, m.index - 80), m.index).replace(/[\s(]+$/, '');
    if (!anchored.test(before)) continue;
    literals(m[1] ?? m[2]).forEach(v => values.add(v));
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

// A replacement constraint often states the domain as OR'd equalities instead of
// an IN list, e.g. CHECK ((col = 'a' AND x IS NULL) OR (col = 'b' AND x IS NULL)).
// A single equality is not accepted as a domain: it may be one branch of a wider
// condition, and treating it as the whole set would narrow the check silently.
function equalityDomain(text, column) {
  const values = new Set();
  for (const m of text.matchAll(/\bCHECK\s*\(/gi)) {
    const open = m.index + m[0].length - 1;
    const body = text.slice(open + 1, matchingParen(text, open));
    const found = [...body.matchAll(new RegExp(`\\b${column}\\s*=\\s*'([A-Za-z0-9_-]+)'`, 'g'))].map(x => x[1]);
    if (found.length >= 2) found.forEach(v => values.add(v));
  }
  return [...values];
}

// Constraint names are not uniform: some end in _check, others are semantic
// (reading_current_publication). A drop counts when the name ends with the
// column or contains it as a whole segment.
function droppedConstraint(statement, table, column) {
  const m = statement.match(/DROP\s+CONSTRAINT\s+(?:IF\s+EXISTS\s+)?([a-z0-9_]+)/i);
  if (!m) return null;
  const name = m[1].toLowerCase();
  const mentionsColumn = name.includes(`_${column}_`) || name.endsWith(`_${column}`);
  return name === `${table}_check` || mentionsColumn ? name : null;
}

// Some migrations drop every CHECK on a column through a PL/pgSQL loop instead of
// naming the constraint: it selects constraints whose definition LIKE '%<column>%'
// and EXECUTEs a DROP on that table. Recognising the idiom keeps a replacement
// from looking like a widening.
function dynamicDrop(text, table, column) {
  return new RegExp(`LIKE\\s*'%${column}%'`, 'i').test(text)
    && new RegExp(`ALTER\\s+TABLE\\s+${table}\\s+DROP\\s+CONSTRAINT`, 'i').test(text);
}

export function databaseStates(files, wanted) {
  const found = new Map();
  let order = 0;
  const push = (key, entry) => {
    if (!found.has(key)) found.set(key, []);
    found.get(key).push({ ...entry, order: order++ });
  };
  const remember = (key, values, source) => {
    if (values.length) push(key, { kind: 'definition', values: [...values].sort(), source });
  };
  for (const [index, { name, text }] of files.entries()) {
    const bodies = tableBodies(text);
    for (const [table, columns] of wanted) {
      const body = bodies.get(table);
      if (body) {
        for (const column of columns) {
          const direct = columnInLists(body, column);
          if (direct.length) { remember(`${table}.${column}`, direct, { name, index }); continue; }
          const slice = columnSlice(body, column);
          const inSlice = slice ? columnInLists(slice, column) : [];
          if (inSlice.length) { remember(`${table}.${column}`, inSlice, { name, index }); continue; }
          const equalities = equalityDomain(body, column);
          if (equalities.length) remember(`${table}.${column}`, equalities, { name, index });
        }
      }
      // The loop form runs before the ADD that follows it in the same file.
      for (const column of columns) {
        if (dynamicDrop(text, table, column)) push(`${table}.${column}`, { kind: 'drop', source: { name, index } });
      }
    }
    for (const m of text.matchAll(/ALTER\s+TABLE\s+(?:IF\s+EXISTS\s+)?(?:public\.)?([a-z0-9_]+)\b[\s\S]*?;/gi)) {
      const table = m[1].toLowerCase();
      const statement = m[0];
      const columns = wanted.get(table);
      if (!columns) continue;
      for (const column of columns) {
        // DROP and ADD can share one statement, so the drop is recorded and the
        // scan continues: the replacement in the same statement is what counts.
        const dropped = droppedConstraint(statement, table, column);
        if (dropped) push(`${table}.${column}`, { kind: 'drop', source: { name, index } });
        const direct = columnInLists(statement, column);
        if (direct.length) { remember(`${table}.${column}`, direct, { name, index }); continue; }
        const equalities = equalityDomain(statement, column);
        if (equalities.length) { remember(`${table}.${column}`, equalities, { name, index }); continue; }
        if (dropped || !new RegExp(`\\b${column}\\b`).test(statement)) continue;
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

// Build output and dependencies hold no production sources; walking them only
// slows the gate down (apps/web keeps node_modules and .next inside the tree).
const SKIP_DIRS = new Set(['tests', 'fixtures', 'target', 'node_modules', '.next', 'dist', 'build', 'vendor']);

export function rustFiles(dir) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) return [];
      return rustFiles(path);
    }
    if (!entry.name.endsWith('.rs')) return [];
    if (/_tests?\.rs$/.test(entry.name)) return [];
    return [path];
  });
}

// Lexed rather than regex-matched: a quote inside a comment or a char literal
// such as '"' desynchronises naive pairing and silently hides later statements.
export function stringLiterals(text) {
  const out = [];
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (c === '/' && text[i + 1] === '/') {
      const end = text.indexOf('\n', i);
      i = end < 0 ? text.length : end + 1;
    } else if (c === '/' && text[i + 1] === '*') {
      let depth = 1;
      i += 2;
      while (i < text.length && depth > 0) {
        if (text[i] === '/' && text[i + 1] === '*') { depth += 1; i += 2; }
        else if (text[i] === '*' && text[i + 1] === '/') { depth -= 1; i += 2; }
        else i += 1;
      }
    } else if (c === 'r' && /^r#*"/.test(text.slice(i, i + 8))) {
      const hashes = /^#*/.exec(text.slice(i + 1))[0];
      const close = `"${hashes}`;
      const start = i + 2 + hashes.length;
      const end = text.indexOf(close, start);
      out.push(text.slice(start, end < 0 ? text.length : end));
      i = end < 0 ? text.length : end + close.length;
    } else if (c === '"') {
      let j = i + 1;
      let value = '';
      while (j < text.length && text[j] !== '"') {
        if (text[j] === '\\') { value += text[j + 1] ?? ''; j += 2; } else { value += text[j]; j += 1; }
      }
      out.push(value);
      i = j + 1;
    } else if (c === "'") {
      if (text[i + 1] === '\\') { const end = text.indexOf("'", i + 2); i = end < 0 ? text.length : end + 1; }
      else if (text[i + 2] === "'") i += 3;
      else i += 1;
    } else {
      i += 1;
    }
  }
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

// Enum bodies are brace-delimited; counting parentheses truncates the list at the
// first tuple variant such as `Blocked(Reason)`.
function matchingBrace(text, open) {
  let depth = 0;
  for (let i = open; i < text.length; i += 1) {
    if (text[i] === '{') depth += 1;
    else if (text[i] === '}') { depth -= 1; if (depth === 0) return i; }
  }
  return text.length;
}

export function enumVariants(path) {
  if (!existsSync(path)) return null;
  const text = readFileSync(path, 'utf8');
  const out = new Map();
  for (const m of text.matchAll(/pub\s+enum\s+([A-Za-z0-9_]+)\s*\{/g)) {
    const open = m.index + m[0].length - 1;
    const body = text.slice(open + 1, matchingBrace(text, open));
    out.set(m[1], [...body.matchAll(/(?:^|,)\s*([A-Z][A-Za-z0-9_]*)\s*(?=[,({]|$)/gm)]
      .map(x => x[1].replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase())
      .sort());
  }
  return out;
}

export function compareEntries(key, states, entries) {
  if (!entries) return `${key}: documented, but no CHECK constraint found in database/migrations`;
  // A later migration drops the old constraint and adds a full new one, so the
  // last event wins. A drop with no readable replacement leaves the domain
  // unknown, which must fail loudly instead of falling back to the old set.
  const lastDrop = entries.filter(e => e.kind === 'drop').pop();
  const definitions = entries.filter(e => e.kind === 'definition');
  if (lastDrop && (!definitions.length || lastDrop.order > definitions[definitions.length - 1].order)) {
    return `${key}: constraint dropped in ${lastDrop.source.name} with no readable replacement; the documented set cannot be verified`;
  }
  // Adding a wider CHECK without dropping the old one does not widen the column:
  // PostgreSQL enforces both constraints, so the effective domain is the intersection.
  let previous = null;
  let droppedSince = false;
  for (const entry of entries) {
    if (entry.kind === 'drop') { droppedSince = true; continue; }
    if (previous && !droppedSince && entry.values.some(v => !previous.values.includes(v))) {
      return `${key}: ${entry.source.name} adds values without dropping the earlier constraint; PostgreSQL keeps their intersection`;
    }
    previous = entry;
    droppedSince = false;
  }
  // Within one file a second, narrower CHECK on the same column is an extra
  // condition, not a new domain: keep the widest of that file. Incomparable
  // sets mean a real conflict.
  const latestIndex = definitions[definitions.length - 1].source.index;
  const latest = definitions.filter(d => d.source.index === latestIndex);
  const widest = latest.reduce((a, b) => (a.values.every(v => b.values.includes(v)) ? b : a));
  const incomparable = latest.filter(s => !s.values.every(v => widest.values.includes(v)));
  if (incomparable.length) {
    const detail = latest.map(d => `${show(d.values)} (${d.source.name})`).join(' vs ');
    return `${key}: database defines incompatible sets: ${detail}`;
  }
  if (eq(widest.values, states)) return null;
  const extra = widest.values.filter(v => !states.includes(v));
  const missing = states.filter(v => !widest.values.includes(v));
  return `${key}: doc ${show(states)} != database ${show(widest.values)} (${widest.source.name})` +
    `${extra.length ? ` | database-only: ${show(extra)}` : ''}${missing.length ? ` | doc-only: ${show(missing)}` : ''}`;
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
    const problem = compareEntries(key, entry.states, db.get(key));
    if (problem) errors.push(problem);
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
