import db from './index.js';
import {
  normalizeNoteRecord,
  normalizeCommentRecord,
  normalizeAuthorRecord,
  normalizeMediaAssetRecord,
} from './recordNormalization.js';

const PAGE_SIZE = 500;

function getChangedRecords(records = [], normalizeRecord) {
  const changed = [];
  for (const record of records) {
    const normalized = normalizeRecord(record);
    if (JSON.stringify(normalized) !== JSON.stringify(record)) {
      changed.push(normalized);
    }
  }
  return changed;
}

async function paginatedBackfill(table, normalizeFn) {
  let offset = 0;
  let totalChanged = 0;
  while (true) {
    const records = await table.offset(offset).limit(PAGE_SIZE).toArray();
    if (records.length === 0) break;
    const changed = getChangedRecords(records, normalizeFn);
    if (changed.length > 0) {
      await table.bulkPut(changed);
      totalChanged += changed.length;
    }
    offset += PAGE_SIZE;
  }
  return totalChanged;
}

export async function backfillLegacyAiReadyFields() {
  const [notes, comments, authors, mediaAssets] = await Promise.all([
    paginatedBackfill(db.notes, normalizeNoteRecord),
    paginatedBackfill(db.comments, normalizeCommentRecord),
    paginatedBackfill(db.authors, normalizeAuthorRecord),
    paginatedBackfill(db.mediaAssets, normalizeMediaAssetRecord),
  ]);

  return {
    notes,
    comments,
    authors,
    mediaAssets,
    total: notes + comments + authors + mediaAssets,
  };
}
