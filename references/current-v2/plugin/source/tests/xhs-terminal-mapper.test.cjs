/**
 * XHS terminal → CaptureSubmissionV2 dark mapper acceptance tests.
 *
 * The terminal fixture is deliberately produced by the public
 * resultPackager. This prevents this boundary test from inventing a second,
 * self-consistent terminal shape.
 */
const { describe, it } = require("node:test");
const assert = require("node:assert/strict");
const crypto = require("node:crypto");

const {
  WORKFLOW_TO_CONTRACT,
  mapTerminalToCaptureSubmissionV2,
  mapRuntimeTerminalToCaptureSubmissionV2,
} = require("../src/workbench/protocol/v2/xhs-terminal-mapper.cjs");
const {
  FIXTURE_BLUEPRINTS,
  CONTRACTS,
  contractHash,
  canonicalJson,
} = require("../src/workbench/protocol/v2/xhs-contracts.cjs");

const PROFILES = Object.keys(WORKFLOW_TO_CONTRACT);
const EXPECTED_WORKFLOW_TO_CONTRACT = {
  list_scan: "xhs.list-scan",
  note_detail: "xhs.note-detail",
  note_full: "xhs.note-full",
  comment_probe: "xhs.comment-probe",
  author_profile: "xhs.author-profile",
  author_links: "xhs.author-links",
};
const STARTED_AT = Date.parse("2026-08-10T10:00:00.000Z");
const FINISHED_AT = Date.parse("2026-08-10T10:05:00.000Z");

function recordsFor(profile) {
  const blueprint = FIXTURE_BLUEPRINTS[WORKFLOW_TO_CONTRACT[profile]];
  return {
    notes: blueprint.recordKinds.includes("note")
      ? [{
          noteId: `n-${profile}-1`,
          platformContentId: `n-${profile}-1`,
          type: profile === "note_detail" || profile === "note_full" ? "video" : "normal",
          title: `Test ${profile}`,
        }]
      : [],
    comments: blueprint.recordKinds.includes("comment")
      ? [{ commentId: `c-${profile}-1`, noteId: `n-${profile}-1`, text: "test comment" }]
      : [],
    authors: blueprint.recordKinds.includes("author")
      ? [{ authorId: `a-${profile}-1`, platformAuthorId: `a-${profile}-1`, name: "test author" }]
      : [],
    mediaAssets: blueprint.mediaPolicy === "metadata_only"
      ? [{
          assetId: `media_n-${profile}-1_cover-1`,
          noteId: `n-${profile}-1`,
          assetType: "image",
          role: "cover",
          ordinal: 0,
          coverProvenance: "platform_explicit",
          sourceUrl: `https://example.com/${profile}.jpg`,
        }]
      : [],
  };
}

async function packagedTerminal(profile, overrides = {}) {
  const { createResultPackager } = await import("../src/workbench/runtime/resultPackager.js");
  const { buildPersistedXhsCaptureReport } = await import("../src/workbench/runtime/xhsCaptureReport.js");
  const records = recordsFor(profile);
  const emitted = records.notes.length + records.comments.length + records.authors.length;
  const status = overrides.status || "done";
  const runRecord = {
    collectionRunId: `run-${profile}`,
    externalTaskId: `job-${profile}-001`,
    externalTaskType: profile,
    platform: "xhs",
    taskType: profile,
    collectionProfile: profile,
    status,
    startedAt: STARTED_AT,
    finishedAt: FINISHED_AT,
    itemsPlanned: records.notes.length + records.comments.length + records.authors.length,
    itemsSucceeded: records.notes.length + records.comments.length + records.authors.length,
    itemsFailed: 0,
    diagnostic: {
      stage: "collecting",
      failureCategory: "none",
      reasonCode: "completed",
      evidence: { profile },
    },
    captureReport: status === "done"
      ? buildPersistedXhsCaptureReport({
          collectionProfile: profile,
          status,
          producerReason: "target_reached",
          counters: {
            requested: emitted,
            discovered: emitted,
            emitted,
            deduplicated: 0,
            failed: 0,
          },
          slotReports: FIXTURE_BLUEPRINTS[WORKFLOW_TO_CONTRACT[profile]].slotIds
            .map((slotId) => ({ slotId, status: "observed", reason: null })),
        })
      : null,
    ...overrides,
  };
  const store = (rows) => ({ getByCollectionRunId: async () => rows });
  const packager = createResultPackager({
    collectionRunStore: {
      getById: async () => runRecord,
      markResultUploadStatus: async () => undefined,
    },
    noteStore: store(records.notes),
    commentStore: store(records.comments),
    authorStore: store(records.authors),
    mediaAssetStore: store(records.mediaAssets),
  });
  return packager.packageByCollectionRunId(runRecord.collectionRunId);
}

function reservationFor(profile, overrides = {}) {
  return {
    jobId: `job-${profile}-001`,
    attemptId: `att-${profile}-001`,
    leaseEpoch: 1,
    platform: "xhs",
    collectionProfile: profile,
    targetKey: `xhs:note/${profile}-test`,
    captureId: `capture-server-${profile}-001`,
    executionPlanVersion: "plan-server-v1",
    ...overrides,
  };
}

function explicitReportFor(profile, terminal) {
  const blueprint = FIXTURE_BLUEPRINTS[WORKFLOW_TO_CONTRACT[profile]];
  const emitted = blueprint.recordKinds.reduce((total, kind) => {
    const key = kind === "note" ? "notes" : kind === "comment" ? "comments" : "authors";
    return total + terminal.records[key].length;
  }, 0);
  return {
    captureTerminal: { state: "completed", reason: "source_exhausted", retryable: false },
    slotReports: blueprint.slotIds.map((slotId) => ({ slotId, status: "observed", reason: null })),
    captureCounters: {
      requested: emitted,
      discovered: emitted,
      emitted,
      deduplicated: 0,
      failed: 0,
    },
  };
}

async function validInput(profile, overrides = {}) {
  const terminal = overrides.terminal || await packagedTerminal(profile);
  return {
    reservation: reservationFor(profile),
    terminal,
    collectorVersion: "2.0.92",
    observedTargetKey: `xhs:note/${profile}-test`,
    ...explicitReportFor(profile, terminal),
    ...overrides,
  };
}

function decodePackage(body) {
  return JSON.parse(Buffer.from(body.capturePackage.packagePayload, "base64").toString("utf8"));
}

function decodeArtifact(artifact) {
  return JSON.parse(Buffer.from(artifact.artifactPayload, "base64").toString("utf8"));
}

describe("workflow contract anchors", () => {
  it("maps exactly six profiles to registered fixed contracts", () => {
    assert.equal(PROFILES.length, 6);
    assert.deepEqual(WORKFLOW_TO_CONTRACT, EXPECTED_WORKFLOW_TO_CONTRACT);
    for (const [profile, contractId] of Object.entries(EXPECTED_WORKFLOW_TO_CONTRACT)) {
      assert.ok(PROFILES.includes(profile));
      assert.ok(CONTRACTS[contractId]);
      assert.ok(FIXTURE_BLUEPRINTS[contractId]);
    }
  });
});

describe("real resultPackager output → CaptureSubmissionV2", () => {
  for (const profile of PROFILES) {
    it(`${profile}: accepts the public packaged terminal shape`, async () => {
      const input = await validInput(profile);
      assert.equal(input.terminal.status, "done");
      assert.equal(typeof input.terminal.finishedAt, "number");

      const result = mapTerminalToCaptureSubmissionV2(input);
      assert.equal(result.ok, true, result.error);
      const { header, capturePackage } = result.body;
      const contractId = WORKFLOW_TO_CONTRACT[profile];
      assert.equal(header.captureId, `capture-server-${profile}-001`);
      assert.equal(header.executionPlanVersion, "plan-server-v1");
      assert.equal(header.contractId, contractId);
      assert.equal(header.contractHash, contractHash(CONTRACTS[contractId]));
      assert.equal(header.observedAt, "2026-08-10T10:05:00.000Z");
      assert.equal(header.report.startedAt, "2026-08-10T10:00:00.000Z");
      assert.equal(header.report.completedAt, "2026-08-10T10:05:00.000Z");
      assert.deepEqual(header.report.terminal, input.captureTerminal);
      assert.deepEqual(header.report.slots, input.slotReports);
      assert.deepEqual(header.report.counters, input.captureCounters);

      const bytes = Buffer.from(capturePackage.packagePayload, "base64");
      assert.equal(capturePackage.contentLength, bytes.length);
      assert.equal(capturePackage.checksumValue, crypto.createHash("sha256").update(bytes).digest("hex"));
      assert.deepEqual(decodePackage(result.body).header, JSON.parse(canonicalJson(header)));
    });

    it(`${profile}: emits only contract record kinds`, async () => {
      const result = mapTerminalToCaptureSubmissionV2(await validInput(profile));
      assert.equal(result.ok, true, result.error);
      const pkg = decodePackage(result.body);
      for (const record of pkg.records) {
        assert.ok(CONTRACTS[WORKFLOW_TO_CONTRACT[profile]].recordKinds.includes(record.recordKind));
        assert.notEqual(record.recordKind, "media");
        assert.notEqual(record.recordKind, "metric");
      }
    });
  }

  it("serializes the current run's actual media records, never fixture media", async () => {
    const input = await validInput("note_detail");
    const result = mapTerminalToCaptureSubmissionV2(input);
    assert.equal(result.ok, true, result.error);
    const artifact = decodePackage(result.body).artifacts.find((item) => item.kind === "media_inventory");
    assert.ok(artifact);
    assert.deepEqual(decodeArtifact(artifact), {
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [{
        subject: {
          kind: "note",
          noteId: "n-note_detail-1",
          platformContentId: "n-note_detail-1",
        },
        slotId: "note:n-note_detail-1:cover:image:0",
        purpose: "cover",
        kind: "image",
        ordinal: 0,
        observedAddress: "https://example.com/note_detail.jpg",
        coverProvenance: "platform_explicit",
      }],
    });
    assert.ok(!JSON.stringify(artifact).includes("fixture-cover"));
  });

  it("rejects comment media until a real producer and persisted source contract exist", async () => {
    const terminal = await packagedTerminal("comment_probe");
    terminal.records.mediaAssets.push({
      assetId: "unexpected-but-observed-media",
      noteId: "n-comment_probe-1",
      commentId: "c-comment_probe-1",
      assetType: "image",
      role: "comment_image",
      ordinal: 0,
      sourceUrl: "https://example.com/observed-comment-media.jpg",
    });
    const input = await validInput("comment_probe", { terminal });
    const result = mapTerminalToCaptureSubmissionV2(input);
    assert.equal(result.ok, false);
    assert.equal(result.reason, "media_subject_mismatch");
  });

  it("is deterministic for the same server identity and packaged terminal", async () => {
    const input = await validInput("note_detail");
    const first = mapTerminalToCaptureSubmissionV2(input);
    const second = mapTerminalToCaptureSubmissionV2(input);
    assert.equal(first.ok, true);
    assert.equal(second.ok, true);
    assert.equal(first.body.capturePackage.checksumValue, second.body.capturePackage.checksumValue);
    assert.equal(first.body.capturePackage.packagePayload, second.body.capturePackage.packagePayload);
  });

  it("deep-snapshots nested terminal and report input", async () => {
    const input = await validInput("note_detail");
    const result = mapTerminalToCaptureSubmissionV2(input);
    assert.equal(result.ok, true, result.error);
    const before = canonicalJson(result.body);
    input.terminal.resultSummary.discoverySummary = { injected: true };
    input.terminal.records.notes[0].title = "MUTATED";
    input.terminal.records.mediaAssets[0].sourceUrl = "https://evil.invalid/mutated";
    input.slotReports[0].status = "invalid";
    input.captureTerminal.reason = "parser_failed";
    assert.equal(canonicalJson(result.body), before);
  });
});

describe("real runtime terminal → CaptureSubmissionV2", () => {
  it("recognizes only the seven audited noteCollector terminal reasons", async () => {
    const {
      buildPersistedXhsCaptureReport,
      XHS_DISCOVERY_SUCCESS_REASONS,
    } = await import("../src/workbench/runtime/xhsCaptureReport.js");
    assert.deepEqual(XHS_DISCOVERY_SUCCESS_REASONS, [
      "target_reached",
      "captured_partial",
      "api_partial",
      "max_rounds_reached",
      "bottom_confirmed",
      "stable_no_new",
      "no_cards_found",
    ]);
    for (const reason of XHS_DISCOVERY_SUCCESS_REASONS) {
      assert.ok(buildPersistedXhsCaptureReport({
        collectionProfile: "list_scan",
        status: "done",
        producerReason: reason,
        counters: { requested: 0, discovered: 0, emitted: 0, deduplicated: 0, failed: 0 },
        slotReports: [{ slotId: "note_list", status: "observed", reason: null }],
      }));
    }
    assert.equal(buildPersistedXhsCaptureReport({
      collectionProfile: "list_scan",
      status: "done",
      producerReason: "source_exhausted",
      counters: { requested: 0, discovered: 0, emitted: 0, deduplicated: 0, failed: 0 },
      slotReports: [{ slotId: "note_list", status: "observed", reason: null }],
    }), null);
  });

  it("batch producers report list/note/comment slots from execution facts, including zero results", async () => {
    const {
      buildXhsBatchNoteCaptureReport,
      buildXhsBatchCommentCaptureReport,
    } = await import("../src/workbench/runtime/xhsCaptureReport.js");
    const emptyList = buildXhsBatchNoteCaptureReport({
      collectionProfile: "list_scan",
      status: "done",
      producerReason: "no_cards_found",
      patch: { itemsPlanned: 20, itemsSucceeded: 0, itemsFailed: 0, totalComments: 0 },
    });
    assert.deepEqual(emptyList.slotReports, [
      { slotId: "note_list", status: "observed", reason: null },
    ]);
    assert.equal(emptyList.captureCounters.emitted, 0);

    const noteDetail = buildXhsBatchNoteCaptureReport({
      collectionProfile: "note_detail",
      status: "done",
      producerReason: "target_reached",
      includeComments: false,
      patch: { itemsPlanned: 1, itemsSucceeded: 1, itemsFailed: 0 },
    });
    assert.deepEqual(noteDetail.slotReports, [
      { slotId: "note", status: "observed", reason: null },
      { slotId: "comments", status: "not_applicable", reason: "not_requested_by_plan" },
    ]);
    assert.equal(buildXhsBatchNoteCaptureReport({
      collectionProfile: "note_full",
      status: "done",
      producerReason: "target_reached",
      includeComments: false,
      patch: { itemsPlanned: 1, itemsSucceeded: 1, itemsFailed: 0 },
    }), null);

    const emptyComments = buildXhsBatchCommentCaptureReport({
      collectionProfile: "comment_probe",
      status: "done",
      patch: { itemsPlanned: 1, itemsSucceeded: 0, itemsFailed: 0, totalComments: 0 },
    });
    assert.deepEqual(emptyComments.slotReports, [
      { slotId: "comments", status: "observed", reason: null },
    ]);
    assert.equal(emptyComments.captureCounters.emitted, 0);
  });

  for (const profile of PROFILES) {
    it(`${profile}: validates and transcribes the persisted producer report`, async () => {
      const terminal = await packagedTerminal(profile);
      const persistedReport = terminal.captureReport;
      const result = mapRuntimeTerminalToCaptureSubmissionV2({
        reservation: reservationFor(profile),
        terminal,
        collectorVersion: "2.0.92",
        observedTargetKey: `xhs:note/${profile}-test`,
      });

      assert.equal(result.ok, true, result.error);
      assert.deepEqual(result.body.header.report.terminal, persistedReport.captureTerminal);
      assert.deepEqual(result.body.header.report.slots, persistedReport.slotReports);
      assert.deepEqual(result.body.header.report.counters, persistedReport.captureCounters);
    });
  }

  it("accepts a producer-proven zero-result workflow without inferring absent", async () => {
    const { buildPersistedXhsCaptureReport } = await import("../src/workbench/runtime/xhsCaptureReport.js");
    const captureReport = buildPersistedXhsCaptureReport({
      collectionProfile: "list_scan",
      status: "done",
      producerReason: "no_cards_found",
      counters: { requested: 10, discovered: 0, emitted: 0, deduplicated: 0, failed: 0 },
      slotReports: [{ slotId: "note_list", status: "observed", reason: null }],
    });
    const terminal = await packagedTerminal("list_scan", { captureReport });
    terminal.records.notes = [];
    terminal.records.mediaAssets = [];
    const result = mapRuntimeTerminalToCaptureSubmissionV2({
      reservation: reservationFor("list_scan"),
      terminal,
      collectorVersion: "2.0.92",
      observedTargetKey: "xhs:note/list_scan-test",
    });

    assert.equal(result.ok, true, result.error);
    assert.equal(result.ok, true, result.error);
    assert.deepEqual(result.body.header.report.slots, [
      { slotId: "note_list", status: "observed", reason: null },
    ]);
    assert.deepEqual(result.body.header.report.counters, {
      requested: 10,
      discovered: 0,
      emitted: 0,
      deduplicated: 0,
      failed: 0,
    });
  });

  it("fails closed when the producer report is missing instead of reading stopReason or record count", async () => {
    const terminal = await packagedTerminal("note_detail");
    terminal.captureReport = null;
    terminal.runRecord.captureReport = null;
    terminal.runRecord.discoverySummary = { stopReason: "target_reached" };
    const result = mapRuntimeTerminalToCaptureSubmissionV2({
      reservation: reservationFor("note_detail"),
      terminal,
      collectorVersion: "2.0.92",
      observedTargetKey: "xhs:note/note_detail-test",
    });

    assert.deepEqual(result, {
      ok: false,
      reason: "missing_captureReport",
      error: "Runtime producer did not persist a captureReport",
    });
  });

  it("submits classified failed Evidence with invalid slots and never calls it success", async () => {
    const { buildPersistedXhsCaptureReport } = await import("../src/workbench/runtime/xhsCaptureReport.js");
    const captureReport = buildPersistedXhsCaptureReport({
      collectionProfile: "note_detail",
      status: "failed",
      failureReason: "parser_failed",
      counters: { requested: 1, discovered: 0, emitted: 0, deduplicated: 0, failed: 1 },
    });
    const terminal = await packagedTerminal("note_detail", {
      status: "failed",
      captureReport,
    });
    terminal.records = { notes: [], comments: [], authors: [], mediaAssets: [] };
    const result = mapRuntimeTerminalToCaptureSubmissionV2({
      reservation: reservationFor("note_detail"),
      terminal,
      collectorVersion: "2.0.92",
      observedTargetKey: "xhs:note/note_detail-test",
    });

    assert.equal(result.ok, true, result.error);
    assert.deepEqual(result.body.header.report.terminal, {
      state: "error",
      reason: "parser_failed",
      retryable: false,
    });
    assert.deepEqual(result.body.header.report.slots, [
      { slotId: "note", status: "invalid", reason: "parser_failed" },
      { slotId: "comments", status: "invalid", reason: "parser_failed" },
    ]);
  });
});

describe("fail-closed source boundaries", () => {
  it("rejects aliases after the real XHS task result sanitizer instead of synthesizing source facts", async () => {
    const { buildWorkbenchResultSummary } = await import("../src/workbench/runtime/taskPoller.js");
    const terminal = await packagedTerminal("note_detail");
    terminal.records = buildWorkbenchResultSummary({
      platform: "xhs",
      records: {
        notes: [{
          noteId: terminal.records.notes[0].noteId,
          contentId: terminal.records.notes[0].noteId,
          contentType: "video",
          title: terminal.records.notes[0].title,
        }],
        comments: [],
        authors: [],
        mediaAssets: [],
      },
    }).records;

    const result = mapTerminalToCaptureSubmissionV2(await validInput("note_detail", { terminal }));
    assert.equal(result.ok, false);
    assert.equal(result.reason, "identity_missing");
  });

  it("rejects unequal identities and missing or unknown note type before packaging", async () => {
    const cases = [
      ["identity_mismatch", (terminal) => { terminal.records.notes[0].platformContentId = "other-note"; }],
      ["identity_missing", (terminal) => { delete terminal.records.notes[0].platformContentId; }],
      ["identity_missing", (terminal) => { delete terminal.records.notes[0].noteId; }],
      ["content_type_missing", (terminal) => { delete terminal.records.notes[0].type; }],
      ["content_type_missing", (terminal) => {
        delete terminal.records.notes[0].type;
        terminal.records.notes[0].contentType = "video";
      }],
      ["content_type_invalid", (terminal) => { terminal.records.notes[0].type = "article"; }],
    ];
    for (const [reason, mutate] of cases) {
      const terminal = await packagedTerminal("note_detail");
      mutate(terminal);
      const result = mapTerminalToCaptureSubmissionV2(await validInput("note_detail", { terminal }));
      assert.equal(result.ok, false);
      assert.equal(result.reason, reason);
    }
  });

  it("rejects a producer author that supplies only one identity", async () => {
    const terminal = await packagedTerminal("author_profile");
    delete terminal.records.authors[0].platformAuthorId;
    const result = mapTerminalToCaptureSubmissionV2(await validInput("author_profile", { terminal }));
    assert.equal(result.ok, false);
    assert.equal(result.reason, "identity_missing");
  });

  it("rejects media whose subject is absent or whose cover proof is ambiguous", async () => {
    const crossSubject = await packagedTerminal("note_detail");
    crossSubject.records.mediaAssets[0].noteId = "other-note";
    assert.equal(
      mapTerminalToCaptureSubmissionV2(await validInput("note_detail", { terminal: crossSubject })).reason,
      "media_subject_mismatch",
    );

    const ambiguousCover = await packagedTerminal("note_detail");
    ambiguousCover.records.mediaAssets[0].coverProvenance = "";
    assert.equal(
      mapTerminalToCaptureSubmissionV2(await validInput("note_detail", { terminal: ambiguousCover })).reason,
      "media_cover_provenance_invalid",
    );
  });

  it("rejects a failed packaged run without source-classified terminal facts", async () => {
    const terminal = await packagedTerminal("note_detail", {
      status: "failed",
      errorMessage: "some opaque failure",
      diagnostic: { failureCategory: "unknown", reasonCode: "unknown" },
    });
    const input = await validInput("note_detail", { terminal });
    delete input.captureTerminal;
    const result = mapTerminalToCaptureSubmissionV2(input);
    assert.deepEqual(result, {
      ok: false,
      reason: "missing_captureTerminal",
      error: "captureTerminal must be explicitly classified at the failure source",
    });
  });

  it("does not infer parser_failed from terminal.status=failed", async () => {
    const terminal = await packagedTerminal("note_detail", { status: "failed" });
    const input = await validInput("note_detail", { terminal, captureTerminal: undefined });
    const result = mapTerminalToCaptureSubmissionV2(input);
    assert.equal(result.ok, false);
    assert.equal(result.reason, "missing_captureTerminal");
  });

  it("accepts an explicitly source-classified failed run without changing its reason", async () => {
    const terminal = await packagedTerminal("note_detail", { status: "failed" });
    const captureTerminal = { state: "error", reason: "network_failed", retryable: true };
    const result = mapTerminalToCaptureSubmissionV2(
      await validInput("note_detail", { terminal, captureTerminal }),
    );
    assert.equal(result.ok, true, result.error);
    assert.deepEqual(result.body.header.report.terminal, captureTerminal);
  });

  it("rejects missing server-issued captureId instead of synthesizing one", async () => {
    const input = await validInput("note_detail");
    input.reservation.captureId = "";
    const result = mapTerminalToCaptureSubmissionV2(input);
    assert.equal(result.ok, false);
    assert.equal(result.reason, "missing_captureId");
  });

  it("rejects missing server executionPlanVersion and ignores no caller override", async () => {
    const input = await validInput("note_detail", { executionPlanVersion: "caller-plan-must-not-apply" });
    input.reservation.executionPlanVersion = "";
    const result = mapTerminalToCaptureSubmissionV2(input);
    assert.equal(result.ok, false);
    assert.equal(result.reason, "missing_executionPlanVersion");
  });

  it("rejects missing expected target, collector version, explicit slots, and explicit counters", async () => {
    for (const mutate of [
      (input) => { input.reservation.targetKey = ""; },
      (input) => { input.collectorVersion = ""; },
      (input) => { delete input.slotReports; },
      (input) => { delete input.captureCounters; },
    ]) {
      const input = await validInput("note_detail");
      mutate(input);
      assert.equal(mapTerminalToCaptureSubmissionV2(input).ok, false);
    }
  });

  it("rejects missing or invalid real packager timestamps; there is no wall-clock fallback", async () => {
    for (const override of [
      { startedAt: 0 },
      { finishedAt: 0 },
      { startedAt: FINISHED_AT + 1 },
    ]) {
      const terminal = await packagedTerminal("note_detail", override);
      const result = mapTerminalToCaptureSubmissionV2(await validInput("note_detail", { terminal }));
      assert.equal(result.ok, false);
      assert.equal(result.reason, "invalid_terminal_timestamps");
    }
  });

  it("rejects slot guesses, duplicate slots, invalid counters, and emitted-count mismatch", async () => {
    const cases = [
      { slotReports: [{ slotId: "not-in-contract", status: "observed", reason: null }] },
      { slotReports: [
        { slotId: "note", status: "observed", reason: null },
        { slotId: "note", status: "observed", reason: null },
      ] },
      { captureCounters: { requested: 1, discovered: 1, emitted: -1, deduplicated: 0, failed: 0 } },
      { captureCounters: { requested: 1, discovered: 1, emitted: 999, deduplicated: 0, failed: 0 } },
    ];
    for (const overrides of cases) {
      const input = await validInput("note_detail", overrides);
      assert.equal(mapTerminalToCaptureSubmissionV2(input).ok, false);
    }
  });

  it("rejects unsupported platform, unknown workflow, and non-final run status", async () => {
    const unsupported = await validInput("note_detail");
    unsupported.reservation.platform = "douyin";
    assert.equal(mapTerminalToCaptureSubmissionV2(unsupported).reason, "unsupported_platform");

    const unknown = await validInput("note_detail");
    unknown.reservation.collectionProfile = "unknown";
    assert.equal(mapTerminalToCaptureSubmissionV2(unknown).reason, "unknown_workflow");

    const running = await packagedTerminal("note_detail", { status: "running" });
    assert.equal(
      mapTerminalToCaptureSubmissionV2(await validInput("note_detail", { terminal: running })).reason,
      "non_terminal_status",
    );
  });

  it("rejects a packaged result from another job or platform", async () => {
    const otherJob = await packagedTerminal("note_detail");
    otherJob.externalTaskId = "another-job";
    assert.equal(
      mapTerminalToCaptureSubmissionV2(await validInput("note_detail", { terminal: otherJob })).reason,
      "terminal_job_mismatch",
    );

    const otherPlatform = await packagedTerminal("note_detail");
    otherPlatform.platform = "douyin";
    assert.equal(
      mapTerminalToCaptureSubmissionV2(await validInput("note_detail", { terminal: otherPlatform })).reason,
      "terminal_platform_mismatch",
    );
  });

  it("does not include request authority fields", async () => {
    const result = mapTerminalToCaptureSubmissionV2(await validInput("note_detail"));
    assert.equal(result.ok, true, result.error);
    const serialized = JSON.stringify(result.body);
    for (const forbidden of ["workspaceId", "receivedAt", "sourcePrincipal", "stationId", "leaseToken"] ) {
      assert.ok(!serialized.includes(forbidden));
    }
  });
});
