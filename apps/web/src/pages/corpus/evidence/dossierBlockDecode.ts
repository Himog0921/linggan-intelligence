import { bool, list, optStr, ShapeError, str, type Json } from '../../../shared/decode';
import { isRecord } from '../../../shared/api';
import type { DossierBlock, DossierIssue, Speaker } from './dossierTypes';

export function choice<const T extends string>(value: unknown, choices: readonly T[], path: string): T {
  const result = choices.find(item => item === value);
  if (result === undefined) throw new ShapeError(path);
  return result;
}
export function count(value: unknown, path: string, minimum = 0): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < minimum) throw new ShapeError(path);
  return value;
}
export function strings(value: unknown, path: string): string[] {
  if (!Array.isArray(value) || value.some(item => typeof item !== 'string')) throw new ShapeError(path);
  return value;
}
export function parseIssue(row: Json): DossierIssue {
  const blockRef = optStr(row, 'blockRef');
  if (['key', 'code', 'message'].some(key => str(row, key) === '') || blockRef === '') throw new ShapeError('issue');
  return { key: str(row, 'key'), code: str(row, 'code'), message: str(row, 'message'),
    resolution: choice(row.resolution, ['reading_edit', 'source_refresh'], 'resolution'), ...(blockRef === null ? {} : { blockRef }) };
}
function speaker(value: unknown): Speaker { return choice(value, ['author', 'other', 'unconfirmed'], 'speaker'); }
function location(row: Json): Pick<DossierBlock, 'bbox' | 'time'> {
  const result: Pick<DossierBlock, 'bbox' | 'time'> = {};
  if (row.bbox !== undefined && row.bbox !== null) {
    const value = row.bbox;
    if (!Array.isArray(value) || value.length !== 4 || value.some(x => typeof x !== 'number' || !Number.isFinite(x) || x < 0 || x > 1)) throw new ShapeError('bbox');
    const [x, y, w, h] = value as [number, number, number, number];
    if (w <= 0 || h <= 0 || x + w > 1.000001 || y + h > 1.000001) throw new ShapeError('bbox');
    result.bbox = [x, y, w, h];
  }
  if (row.time !== undefined && row.time !== null) {
    if (!isRecord(row.time)) throw new ShapeError('time');
    const startMs = count(row.time.startMs, 'time.startMs'); const endMs = count(row.time.endMs, 'time.endMs');
    if (endMs < startMs || Object.keys(row.time).some(key => !['startMs', 'endMs'].includes(key))) throw new ShapeError('time');
    result.time = { startMs, endMs };
  }
  return result;
}
export function parseBlock(row: Json): DossierBlock {
  const ref = str(row, 'ref'); if (ref === '') throw new ShapeError('ref');
  const kind = choice(row.kind, ['heading', 'paragraph', 'quote', 'gap', 'list', 'table', 'dialogue'], 'kind');
  if (Object.keys(row).some(key => !['ref', 'kind', 'text', 'items', 'ordered', 'start', 'columns', 'rows', 'turns', 'speaker', 'bbox', 'time', 'issues', 'level'].includes(key))) throw new ShapeError('block.field');
  if ((kind !== 'list' && (row.ordered !== undefined || row.start !== undefined)) || (kind !== 'heading' && row.level !== undefined)) throw new ShapeError('block.content');
  const base = { ref, ...location(row), ...(row.speaker === undefined ? {} : { speaker: speaker(row.speaker) }), ...(row.issues === undefined ? {} : { issues: list(row, 'issues', parseIssue) }) };
  const shapes = ['text', 'items', 'columns', 'rows', 'turns'].filter(key => row[key] !== undefined);
  const allowed = kind === 'table' ? ['columns', 'rows'] : kind === 'list' ? ['items'] : kind === 'dialogue' ? ['turns'] : ['text'];
  if (shapes.length !== allowed.length || shapes.some(key => !allowed.includes(key))) throw new ShapeError('block.content');
  if (kind === 'list') {
    const start = row.start === undefined ? undefined : count(row.start, 'start', 1);
    const items = strings(row.items, 'items'); if (items.length === 0) throw new ShapeError('items');
    return { ...base, kind, items, ordered: bool(row, 'ordered'), ...(start === undefined ? {} : { start }) };
  }
  if (kind === 'table') {
    const columns = strings(row.columns, 'columns');
    if (columns.length === 0 || !Array.isArray(row.rows)) throw new ShapeError('table');
    const rows = row.rows.map((value, i) => { const cells = strings(value, `rows[${i}]`); if (cells.length !== columns.length) throw new ShapeError('rows'); return cells; });
    return { ...base, kind, columns, rows };
  }
  if (kind === 'dialogue') {
    const turns = list(row, 'turns', turn => { if (Object.keys(turn).some(key => !['speaker', 'text'].includes(key))) throw new ShapeError('turn'); return { speaker: speaker(turn.speaker), text: str(turn, 'text') }; });
    if (turns.length === 0) throw new ShapeError('turns');
    return { ...base, kind, turns };
  }
  if (kind === 'heading') { const level = row.level === undefined ? undefined : count(row.level, 'level', 1); if (level !== undefined && level > 6) throw new ShapeError('level'); return { ...base, kind, text: str(row, 'text'), ...(level === undefined ? {} : { level }) }; }
  return { ...base, kind, text: str(row, 'text') };
}
export function parseBlocks(row: Json, field = 'blocks'): DossierBlock[] {
  const blocks = list(row, field, parseBlock);
  if (new Set(blocks.map(block => block.ref)).size !== blocks.length) throw new ShapeError('blocks.ref');
  return blocks;
}
