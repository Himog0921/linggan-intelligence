const {
  CONTRACTS,
  EXPECTED_CONTRACT_HASHES,
  buildCaptureSubmissionBodyV2,
  canonicalJson,
  contractHash,
  sha256Hex,
} = require('./xhs-contracts.cjs');
const { validateXhsRecordPayload } = require('./xhs-source-contract.cjs');

const KIND_CONFIG = Object.freeze({
  note: Object.freeze({ contractId: 'xhs.list-scan', slotId: 'note_list' }),
  comment: Object.freeze({ contractId: 'xhs.comment-probe', slotId: 'comments' }),
  author: Object.freeze({ contractId: 'xhs.author-profile', slotId: 'author' }),
});

function fail(reason, error) {
  return { ok: false, reason, error };
}

function nonEmpty(value) {
  return typeof value === 'string' && value.trim() === value && value.length > 0;
}

function observedAtFrom(record) {
  const source = record?.observedAt ?? record?.collectedAt ?? record?.capturedAt;
  const date = typeof source === 'number' || typeof source === 'string' ? new Date(source) : null;
  if (!date || Number.isNaN(date.getTime())) return null;
  return date.toISOString();
}

function externalId(record, kind) {
  if (kind === 'note') return record.noteId;
  if (kind === 'comment') return record.commentId;
  return record.authorId;
}

function targetKey(record, kind) {
  if (kind === 'note') return `xhs:note/${record.noteId}`;
  if (kind === 'comment') return `xhs:note/${record.noteId}/comment/${record.commentId}`;
  return `xhs:author/${record.authorId}`;
}

function metadataText(metadata, key) {
  const value = metadata && typeof metadata === 'object' ? metadata[key] : null;
  return typeof value === 'string' ? value : '';
}

function getRecordGroup(records) {
  const groups = [
    ['note', Array.isArray(records?.notes) ? records.notes : []],
    ['comment', Array.isArray(records?.comments) ? records.comments : []],
    ['author', Array.isArray(records?.authors) ? records.authors : []],
  ].filter(([, rows]) => rows.length > 0);
  if (groups.length === 0) return fail('manual_import_empty', 'manual import has no records');
  if (groups.length !== 1) {
    return fail(
      'manual_import_mixed_record_kinds',
      'one manual import package must use exactly one registered CollectionContract',
    );
  }
  return { ok: true, kind: groups[0][0], rows: groups[0][1] };
}

function buildXhsManualImportSubmissionV2({ records = {}, metadata = {}, collectorVersion = '' } = {}) {
  if (!nonEmpty(collectorVersion)) {
    return fail('collector_version_missing', 'running plugin version is required');
  }
  const group = getRecordGroup(records);
  if (!group.ok) return group;
  const config = KIND_CONFIG[group.kind];
  const contract = CONTRACTS[config.contractId];
  const computedContractHash = contractHash(contract);
  if (computedContractHash !== EXPECTED_CONTRACT_HASHES[config.contractId]) {
    return fail('contract_hash_anchor_mismatch', `contract hash mismatch for ${config.contractId}`);
  }

  const mappedRecords = [];
  for (let sequence = 0; sequence < group.rows.length; sequence += 1) {
    const source = group.rows[sequence];
    if (source?.platform !== 'xhs') {
      return fail(
        `manual_import_platform_unsupported:${String(source?.platform || 'missing')}`,
        'manual import V2 currently accepts XHS records only',
      );
    }
    const validation = validateXhsRecordPayload(group.kind, source);
    if (!validation.ok) return fail(validation.reason, `${validation.path}: ${validation.reason}`);
    const observedAt = observedAtFrom(source);
    if (!observedAt) return fail('observed_at_missing', 'manual import record requires a real observed timestamp');
    const id = externalId(source, group.kind);
    mappedRecords.push({
      idempotencyKey: `manual:${group.kind}:${id}:${sha256Hex(canonicalJson(source))}`,
      recordKind: group.kind,
      platform: 'xhs',
      targetKey: targetKey(source, group.kind),
      externalRecordId: id,
      sequence,
      payload: JSON.parse(canonicalJson(source)),
      observedAt,
    });
  }

  const timestamps = mappedRecords.map((record) => record.observedAt).sort();
  const completedAt = timestamps[timestamps.length - 1];
  const captureSeed = {
    contractId: config.contractId,
    records: mappedRecords,
    source: 'plugin_manual_sync',
  };
  const captureId = `manual-${sha256Hex(canonicalJson(captureSeed))}`;
  const sourceSummary = `plugin manual sync: ${group.kind} (${mappedRecords.length})`;
  const slots = [{ slotId: config.slotId, status: 'observed', reason: null }];
  if (group.kind === 'author') {
    slots.push({ slotId: 'note_list', status: 'not_applicable', reason: 'manual_author_only' });
  }
  const header = {
    protocolVersion: 'capture-submission/v2',
    captureId,
    platform: 'xhs',
    target: {
      expectedTargetKey: `xhs:manual-import/${group.kind}`,
      observedTargetKey: null,
    },
    observedAt: completedAt,
    collectorVersion,
    contractId: contract.id,
    contractVersion: contract.version,
    contractHash: computedContractHash,
    report: {
      startedAt: timestamps[0],
      completedAt,
      terminal: { state: 'completed', reason: 'source_exhausted', retryable: false },
      slots,
      counters: {
        requested: mappedRecords.length,
        discovered: mappedRecords.length,
        emitted: mappedRecords.length,
        deduplicated: 0,
        failed: 0,
      },
      diagnostics: {
        source: 'plugin_manual_sync',
        tag: metadataText(metadata, 'tag'),
        operator: metadataText(metadata, 'operator'),
      },
    },
    ingressKind: 'manual_import',
    sourceSummary,
  };
  return {
    ok: true,
    body: buildCaptureSubmissionBodyV2({ header, records: mappedRecords, artifacts: [] }),
    recordKind: group.kind,
    recordCount: mappedRecords.length,
  };
}

function buildXhsManualImportSubmissionsV2({ records = {}, metadata = {}, collectorVersion = '' } = {}) {
  const rowsFor = (key) => {
    const value = records && typeof records === 'object' ? records[key] : null;
    return Array.isArray(value) ? value : [];
  };
  const groups = [
    { key: 'notes', rows: rowsFor('notes') },
    { key: 'comments', rows: rowsFor('comments') },
    { key: 'authors', rows: rowsFor('authors') },
  ].filter((group) => group.rows.length > 0);
  if (groups.length === 0) return fail('manual_import_empty', 'manual import has no records');

  const submissions = [];
  for (const { key, rows } of groups) {
    const groupedRecords = { notes: [], comments: [], authors: [] };
    if (key === 'notes') groupedRecords.notes = rows;
    else if (key === 'comments') groupedRecords.comments = rows;
    else if (key === 'authors') groupedRecords.authors = rows;
    else return fail('manual_import_record_group_invalid', 'unknown record group');
    const mapped = buildXhsManualImportSubmissionV2({
      records: groupedRecords,
      metadata,
      collectorVersion,
    });
    if (!mapped.ok) return mapped;
    submissions.push(mapped);
  }
  return { ok: true, submissions };
}

module.exports = {
  buildXhsManualImportSubmissionV2,
  buildXhsManualImportSubmissionsV2,
};
