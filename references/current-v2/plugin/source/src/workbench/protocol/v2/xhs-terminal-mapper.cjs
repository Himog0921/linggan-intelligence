/**
 * XHS terminal → V2 CaptureSubmission mapper (B1-B-12).
 *
 * taskPoller calls this mapper for final XHS runs. It validates and transcribes
 * the captureReport already persisted by the workflow producer; it never
 * derives terminal, slot, or counter facts from record counts or aliases.
 */

const {
  CONTRACTS,
  FIXTURE_BLUEPRINTS,
  EXPECTED_CONTRACT_HASHES,
  contractHash,
  canonicalJson,
  buildCaptureSubmissionBodyV2,
  mediaInventoryArtifactFromCandidates,
} = require("./xhs-contracts.cjs");
const {
  validateXhsRecordPayload,
  buildXhsMediaInventoryV2,
} = require("./xhs-source-contract.cjs");

const WORKFLOW_TO_CONTRACT = Object.freeze({
  list_scan: "xhs.list-scan",
  note_detail: "xhs.note-detail",
  note_full: "xhs.note-full",
  comment_probe: "xhs.comment-probe",
  author_profile: "xhs.author-profile",
  author_links: "xhs.author-links",
});

const FINAL_RUN_STATUSES = new Set(["done", "stopped", "failed", "canceled", "rejected"]);
const TERMINAL_STATES = new Set(["completed", "blocked", "cancelled", "error"]);
const TERMINAL_REASONS = new Set([
  "source_exhausted",
  "limit_reached",
  "target_missing",
  "login_required",
  "platform_blocked",
  "parser_failed",
  "network_failed",
  "user_cancelled",
]);
const SLOT_STATUSES = new Set(["observed", "absent", "unavailable", "not_applicable", "invalid"]);
const COUNTER_KEYS = ["requested", "discovered", "emitted", "deduplicated", "failed"];

/**
 * @param {string} reason
 * @param {string} error
 * @returns {{ok: false, reason: string, error: string}}
 */
function fail(reason, error) {
  return { ok: false, reason, error };
}

function isPlainObject(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function nonEmptyString(value) {
  return typeof value === "string" && value.trim() === value && value.length > 0;
}

function jsonSnapshot(value) {
  return JSON.parse(canonicalJson(value));
}

function timestampFromMillis(value) {
  if (!Number.isFinite(value) || !Number.isInteger(value) || value <= 0) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date.toISOString();
}

/** @returns {{ok: true, value: Object} | {ok: false, reason: string, error: string}} */
function validateCaptureTerminal(value) {
  if (!isPlainObject(value)) {
    return fail(
      "missing_captureTerminal",
      "captureTerminal must be explicitly classified at the failure source",
    );
  }
  if (
    !TERMINAL_STATES.has(value.state)
    || !TERMINAL_REASONS.has(value.reason)
    || typeof value.retryable !== "boolean"
    || Object.keys(value).some((key) => !["state", "reason", "retryable"].includes(key))
  ) {
    return fail("invalid_captureTerminal", "captureTerminal is not a valid explicit V2 terminal classification");
  }
  return { ok: true, value: jsonSnapshot(value) };
}

/** @returns {{ok: true, value: Array<Object>} | {ok: false, reason: string, error: string}} */
function validateSlotReports(value, expectedSlotIds) {
  if (!Array.isArray(value)) {
    return fail("missing_slotReports", "slotReports must be supplied by the collection workflow");
  }
  const expected = new Set(expectedSlotIds);
  const seen = new Set();
  for (const slot of value) {
    if (
      !isPlainObject(slot)
      || !nonEmptyString(slot.slotId)
      || !expected.has(slot.slotId)
      || seen.has(slot.slotId)
      || !SLOT_STATUSES.has(slot.status)
      || (slot.reason !== null && !nonEmptyString(slot.reason))
      || Object.keys(slot).some((key) => !["slotId", "status", "reason"].includes(key))
    ) {
      return fail("invalid_slotReports", "slotReports must explicitly and exactly cover the selected contract slots");
    }
    seen.add(slot.slotId);
  }
  if (seen.size !== expected.size) {
    return fail("invalid_slotReports", "slotReports must explicitly and exactly cover the selected contract slots");
  }
  return { ok: true, value: jsonSnapshot(value) };
}

/** @returns {{ok: true, value: Object} | {ok: false, reason: string, error: string}} */
function validateCounters(value, emittedRecords) {
  if (!isPlainObject(value)) {
    return fail("missing_captureCounters", "captureCounters must be supplied by the collection workflow");
  }
  if (
    Object.keys(value).length !== COUNTER_KEYS.length
    || COUNTER_KEYS.some((key) => !Number.isSafeInteger(value[key]) || value[key] < 0)
  ) {
    return fail("invalid_captureCounters", "captureCounters must contain five explicit non-negative integers");
  }
  if (value.emitted !== emittedRecords) {
    return fail("captureCounters_emitted_mismatch", "captureCounters.emitted must equal the emitted RawRecord count");
  }
  return { ok: true, value: jsonSnapshot(value) };
}

function externalRecordId(record, kind) {
  if (!isPlainObject(record)) return null;
  const candidates = kind === "note"
    ? [record.noteId, record.platformContentId, record.id]
    : kind === "comment"
      ? [record.commentId, record.id]
      : [record.authorId, record.platformAuthorId, record.userId, record.id];
  for (const candidate of candidates) {
    if (nonEmptyString(candidate)) return candidate;
  }
  return null;
}

function recordArray(records, kind) {
  if (kind === "note") return records.notes;
  if (kind === "comment") return records.comments;
  return records.authors;
}

function buildRecords(terminalRecords, blueprint, captureId, targetKey, observedAt) {
  const output = [];
  let sequence = 0;
  for (const kind of blueprint.recordKinds) {
    for (const sourceRecord of recordArray(terminalRecords, kind)) {
      const sourceValidation = validateXhsRecordPayload(kind, sourceRecord);
      if (!sourceValidation.ok) {
        throw Object.assign(
          new Error(`${sourceValidation.path}: ${sourceValidation.reason}`),
          { reason: sourceValidation.reason },
        );
      }
      const payload = jsonSnapshot(sourceRecord);
      const externalId = externalRecordId(sourceRecord, kind);
      output.push({
        idempotencyKey: `${captureId}:${kind}:${externalId || `sequence-${sequence}`}`,
        recordKind: kind,
        platform: "xhs",
        targetKey,
        externalRecordId: externalId,
        sequence,
        payload,
        observedAt,
      });
      sequence += 1;
    }
  }
  return output;
}

/**
 * @param {object} input
 * @returns {{ok: true, body: object} | {ok: false, reason: string, error: string}}
 */
function mapTerminalToCaptureSubmissionV2(input) {
  if (!isPlainObject(input)) return fail("invalid_input", "mapper input must be an object");
  if (!isPlainObject(input.reservation)) return fail("invalid_reservation", "reservation is required");
  if (!isPlainObject(input.terminal)) return fail("invalid_terminal", "resultPackager terminal is required");

  let reservation;
  let terminal;
  try {
    reservation = jsonSnapshot(input.reservation);
    terminal = jsonSnapshot(input.terminal);
  } catch (error) {
    return fail("invalid_json_input", error instanceof Error ? error.message : "input is not valid JSON");
  }

  if (!nonEmptyString(reservation.jobId)) return fail("missing_jobId", "reservation.jobId is required");
  if (!nonEmptyString(reservation.attemptId)) return fail("missing_attemptId", "reservation.attemptId is required");
  if (!Number.isSafeInteger(reservation.leaseEpoch) || reservation.leaseEpoch < 0) {
    return fail("missing_leaseEpoch", "reservation.leaseEpoch must be a non-negative integer");
  }
  if (reservation.platform !== "xhs") {
    return fail("unsupported_platform", `platform must be \"xhs\", got ${JSON.stringify(reservation.platform)}`);
  }
  if (!nonEmptyString(reservation.captureId)) {
    return fail("missing_captureId", "reservation.captureId must be the server-issued opaque capture identity");
  }
  if (!nonEmptyString(reservation.executionPlanVersion)) {
    return fail("missing_executionPlanVersion", "reservation.executionPlanVersion must come from ExecutionJob");
  }
  if (!nonEmptyString(reservation.targetKey)) {
    return fail("missing_targetKey", "reservation.targetKey must come from the server execution plan");
  }
  if (!nonEmptyString(input.collectorVersion)) {
    return fail("missing_collectorVersion", "collectorVersion must come from the running plugin version");
  }
  if (input.observedTargetKey !== null && !nonEmptyString(input.observedTargetKey)) {
    return fail("invalid_observedTargetKey", "observedTargetKey must be a non-empty string or null");
  }

  const profile = nonEmptyString(reservation.collectionProfile) ? reservation.collectionProfile : "";
  const contractId = WORKFLOW_TO_CONTRACT[profile];
  if (!contractId) return fail("unknown_workflow", `Unknown collectionProfile: ${JSON.stringify(profile)}`);
  const contract = CONTRACTS[contractId];
  const blueprint = FIXTURE_BLUEPRINTS[contractId];
  if (!contract || !blueprint) return fail("contract_not_found", `Contract not registered: ${contractId}`);
  const computedHash = contractHash(contract);
  if (computedHash !== EXPECTED_CONTRACT_HASHES[contractId]) {
    return fail("contract_hash_anchor_mismatch", `Contract hash mismatch for ${contractId}`);
  }

  if (!nonEmptyString(terminal.status) || !FINAL_RUN_STATUSES.has(terminal.status)) {
    return fail("non_terminal_status", `resultPackager status is not final: ${JSON.stringify(terminal.status)}`);
  }
  if (terminal.platform !== reservation.platform) {
    return fail("terminal_platform_mismatch", "resultPackager platform must match the server reservation");
  }
  if (!nonEmptyString(terminal.externalTaskId) || terminal.externalTaskId !== reservation.jobId) {
    return fail("terminal_job_mismatch", "resultPackager externalTaskId must match reservation.jobId");
  }
  const startedAt = timestampFromMillis(terminal.startedAt);
  const completedAt = timestampFromMillis(terminal.finishedAt);
  if (!startedAt || !completedAt || terminal.finishedAt < terminal.startedAt) {
    return fail("invalid_terminal_timestamps", "resultPackager startedAt/finishedAt must be ordered epoch milliseconds");
  }
  if (!isPlainObject(terminal.records)) {
    return fail("invalid_terminal_records", "resultPackager records object is required");
  }
  for (const key of ["notes", "comments", "authors", "mediaAssets"]) {
    if (!Array.isArray(terminal.records[key])) {
      return fail("invalid_terminal_records", `resultPackager records.${key} must be an array`);
    }
  }

  const terminalResult = validateCaptureTerminal(input.captureTerminal);
  if (terminalResult.ok === false) return terminalResult;
  const slotsResult = validateSlotReports(input.slotReports, blueprint.slotIds);
  if (slotsResult.ok === false) return slotsResult;

  let records;
  let mediaAssets;
  let diagnostics;
  try {
    records = buildRecords(
      terminal.records,
      blueprint,
      reservation.captureId,
      reservation.targetKey,
      completedAt,
    );
    mediaAssets = buildXhsMediaInventoryV2(terminal.records).candidates;
    diagnostics = jsonSnapshot({
      terminalDiagnostic: terminal.diagnostic ?? null,
      resultSummary: terminal.resultSummary ?? {},
    });
  } catch (error) {
    return fail(
      typeof error?.reason === "string" ? error.reason : "invalid_terminal_payload",
      error instanceof Error ? error.message : "terminal payload is not valid JSON",
    );
  }

  const countersResult = validateCounters(input.captureCounters, records.length);
  if (countersResult.ok === false) return countersResult;

  const header = {
    protocolVersion: "capture-submission/v2",
    captureId: reservation.captureId,
    platform: "xhs",
    target: {
      expectedTargetKey: reservation.targetKey,
      observedTargetKey: input.observedTargetKey,
    },
    observedAt: completedAt,
    collectorVersion: input.collectorVersion,
    contractId: contract.id,
    contractVersion: contract.version,
    contractHash: computedHash,
    report: {
      startedAt,
      completedAt,
      terminal: terminalResult.value,
      slots: slotsResult.value,
      counters: countersResult.value,
      diagnostics,
    },
    ingressKind: "execution",
    jobId: reservation.jobId,
    attemptId: reservation.attemptId,
    leaseEpoch: reservation.leaseEpoch,
    executionPlanVersion: reservation.executionPlanVersion,
  };

  const artifacts = mediaAssets.length > 0
    ? [mediaInventoryArtifactFromCandidates(mediaAssets)]
    : [];
  const body = buildCaptureSubmissionBodyV2({ header, records, artifacts });
  return { ok: true, body };
}

function mapRuntimeTerminalToCaptureSubmissionV2(input) {
  if (!isPlainObject(input) || !isPlainObject(input.reservation)) {
    return fail("invalid_mapper_input", "Runtime mapper input and reservation must be objects");
  }
  const contractId = WORKFLOW_TO_CONTRACT[input.reservation.collectionProfile];
  const blueprint = contractId && FIXTURE_BLUEPRINTS[contractId];
  if (!blueprint) {
    return fail("unsupported_collection_profile", "Runtime reservation collectionProfile is not registered");
  }
  const report = input.terminal?.captureReport;
  if (!isPlainObject(report)) {
    return fail("missing_captureReport", "Runtime producer did not persist a captureReport");
  }
  if (!isPlainObject(report.producer)
    || report.producer.collectionProfile !== input.reservation.collectionProfile
    || report.producer.status !== input.terminal.status
    || !nonEmptyString(report.producer.reason)) {
    return fail("invalid_captureReport_source", "captureReport producer identity does not match the terminal run");
  }
  return mapTerminalToCaptureSubmissionV2({
    ...input,
    captureTerminal: report.captureTerminal,
    slotReports: report.slotReports,
    captureCounters: report.captureCounters,
  });
}

module.exports = {
  WORKFLOW_TO_CONTRACT,
  mapTerminalToCaptureSubmissionV2,
  mapRuntimeTerminalToCaptureSubmissionV2,
};
