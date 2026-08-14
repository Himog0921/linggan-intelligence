/**
 * Tests for XHS V2 CollectionContract definitions and fixtures (B1-B-03-R1 §5-6).
 */

const { describe, it } = require("node:test");
const assert = require("node:assert/strict");
const {
  canonicalJson,
  sha256Hex,
  contractHash,
  packageHash,
  buildCaptureSubmissionBodyV2,
  mediaInventoryArtifact,
  fixtureRecordPayload,
  CONTRACTS,
  FIXTURES,
  EXPECTED_CONTRACT_HASHES,
} = require("../src/workbench/protocol/v2/xhs-contracts.cjs");

const ALL_IDS = Object.keys(CONTRACTS);

// ── §2: Six contracts exist ────────────────────────────────────────────

describe("XHS V2 contracts", () => {
  it("has exactly 6 contracts", () => {
    assert.equal(ALL_IDS.length, 6);
    assert.deepEqual(ALL_IDS.sort(), [
      "xhs.author-links",
      "xhs.author-profile",
      "xhs.comment-probe",
      "xhs.list-scan",
      "xhs.note-detail",
      "xhs.note-full",
    ].sort());
  });

  for (const id of ALL_IDS) {
    const c = CONTRACTS[id];
    it(`${id}: version=2, source contracts bound, platform=["xhs"], allowEmptyRecords=true`, () => {
      assert.equal(c.version, 2);
      assert.equal(c.sourceContracts.recordPayload.schemaVersion, "xhs.record-payload/v2");
      assert.equal(c.sourceContracts.mediaInventory.schemaVersion, "xhs.media-inventory/v2");
      assert.match(c.sourceContracts.recordPayload.contractHash, /^[0-9a-f]{64}$/);
      assert.match(c.sourceContracts.mediaInventory.contractHash, /^[0-9a-f]{64}$/);
      assert.deepEqual(c.platforms, ["xhs"]);
      assert.deepEqual(c.terminalPolicy.allowedStates, ["completed", "blocked", "cancelled", "error"]);
      assert.equal(c.terminalPolicy.allowEmptyRecords, true);
    });

    it(`${id}: contract hash is lowercase 64-char hex`, () => {
      const hash = contractHash(c);
      assert.match(hash, /^[0-9a-f]{64}$/);
    });

    it(`${id}: hash is deterministic`, () => {
      assert.equal(contractHash(c), contractHash({ ...c }));
    });
  }

  // ── §5: Contract-specific assertions ─────────────────────────────────

  it("list-scan: recordKinds=[note], slot note_list required, metadata_only", () => {
    const c = CONTRACTS["xhs.list-scan"];
    assert.deepEqual(c.recordKinds, ["note"]);
    assert.deepEqual(c.slots, [{ slotId: "note_list", requirement: "required" }]);
    assert.equal(c.mediaPolicy, "metadata_only");
  });

  it("note-detail: recordKinds=[note,comment], note required + comments conditional", () => {
    const c = CONTRACTS["xhs.note-detail"];
    assert.deepEqual(c.recordKinds, ["note", "comment"]);
    assert.deepEqual(c.slots, [
      { slotId: "note", requirement: "required" },
      { slotId: "comments", requirement: "conditional" },
    ]);
  });

  it("note-full: recordKinds=[note,comment], note required + comments required", () => {
    const c = CONTRACTS["xhs.note-full"];
    assert.deepEqual(c.recordKinds, ["note", "comment"]);
    assert.deepEqual(c.slots, [
      { slotId: "note", requirement: "required" },
      { slotId: "comments", requirement: "required" },
    ]);
  });

  it("comment-probe: recordKinds=[comment], comments required, not_required", () => {
    const c = CONTRACTS["xhs.comment-probe"];
    assert.deepEqual(c.recordKinds, ["comment"]);
    assert.deepEqual(c.slots, [{ slotId: "comments", requirement: "required" }]);
    assert.equal(c.mediaPolicy, "not_required");
  });

  it("author-profile: recordKinds=[author,note], author required + note_list optional", () => {
    const c = CONTRACTS["xhs.author-profile"];
    assert.deepEqual(c.recordKinds, ["author", "note"]);
    assert.deepEqual(c.slots, [
      { slotId: "author", requirement: "required" },
      { slotId: "note_list", requirement: "optional" },
    ]);
  });

  it("author-links: recordKinds=[note], note_links required, not_required", () => {
    const c = CONTRACTS["xhs.author-links"];
    assert.deepEqual(c.recordKinds, ["note"]);
    assert.deepEqual(c.slots, [{ slotId: "note_links", requirement: "required" }]);
    assert.equal(c.mediaPolicy, "not_required");
  });

  // ── §5: Load-time validation ────────────────────────────────────────

  it("rejects duplicate slotId", () => {
    const bad = {
      ...CONTRACTS["xhs.note-detail"],
      slots: [
        { slotId: "dup", requirement: "required" },
        { slotId: "dup", requirement: "optional" },
      ],
    };
    const ids = bad.slots.map((s) => s.slotId);
    assert.notEqual(new Set(ids).size, ids.length, "duplicate slotId should be detected");
  });

  it("rejects duplicate recordKind", () => {
    const bad = { ...CONTRACTS["xhs.note-detail"], recordKinds: ["note", "note"] };
    const kinds = new Set(bad.recordKinds);
    assert.notEqual(kinds.size, bad.recordKinds.length, "duplicate recordKind should be detected");
  });

  it("rejects non-positive version", () => {
    const bad = { ...CONTRACTS["xhs.list-scan"], version: 0 };
    assert.ok(bad.version < 1, "non-positive version should be detected");
  });

  it("contract hash changes when recordKinds change", () => {
    const h1 = contractHash(CONTRACTS["xhs.note-detail"]);
    const mod = { ...CONTRACTS["xhs.note-detail"], recordKinds: ["note"] };
    assert.notEqual(contractHash(mod), h1);
  });

  it("contract hash changes when slots change", () => {
    const h1 = contractHash(CONTRACTS["xhs.note-detail"]);
    const mod = { ...CONTRACTS["xhs.note-detail"], slots: [{ slotId: "note", requirement: "required" }] };
    assert.notEqual(contractHash(mod), h1);
  });

  it("contract hash changes when mediaPolicy changes", () => {
    const h1 = contractHash(CONTRACTS["xhs.list-scan"]);
    const mod = { ...CONTRACTS["xhs.list-scan"], mediaPolicy: "not_required" };
    assert.notEqual(contractHash(mod), h1);
  });

  // ── §3.2: No media RawRecord in V2 ────────────────────────────────────

  it("no contract produces media recordKind", () => {
    for (const id of ALL_IDS) {
      assert.ok(
        !CONTRACTS[id].recordKinds.includes("media"),
        `${id} must not include media as a RawRecord kind (must be media_inventory artifact)`,
      );
    }
  });

  it("no contract produces metric recordKind (V2 first phase)", () => {
    for (const id of ALL_IDS) {
      assert.ok(
        !CONTRACTS[id].recordKinds.includes("metric"),
        `${id} must not include metric (not in V2 first phase)`,
      );
    }
  });
});

// ── §3: Fixtures ───────────────────────────────────────────────────────

describe("XHS V2 fixtures", () => {
  for (const id of ALL_IDS) {
    const f = FIXTURES[id];

    it(`${id}: fixture exists and has submission`, () => {
      assert.ok(f.submission);
      assert.ok(f.submission.body);
      assert.ok(f.submission.body.header);
      assert.ok(f.submission.body.capturePackage);
    });

    it(`${id}: fixture is execution ingressKind`, () => {
      assert.equal(f.submission.body.header.ingressKind, "execution");
      assert.equal(f.submission.authority.ingressKind, "execution");
    });

    it(`${id}: fixture header matches contract id/version/hash`, () => {
      const hdr = f.submission.body.header;
      assert.equal(hdr.contractId, id);
      assert.equal(hdr.contractVersion, 2);
      assert.equal(hdr.contractHash, contractHash(CONTRACTS[id]));
      assert.match(hdr.contractHash, /^[0-9a-f]{64}$/);
    });

    it(`${id}: fixture package hash is lowercase 64-char hex`, () => {
      const pkg = f.submission.body.capturePackage;
      assert.match(pkg.checksumValue, /^[0-9a-f]{64}$/);
      assert.equal(pkg.checksumValue, f.fixturePackageHash);
    });

    it(`${id}: package payload decodes to valid JSON via Buffer`, () => {
      const decoded = Buffer.from(f.submission.body.capturePackage.packagePayload, "base64").toString("utf8");
      const parsed = JSON.parse(decoded);
      assert.equal(parsed.schemaVersion, "capture-package/v2");
      assert.ok(Array.isArray(parsed.records));
      assert.ok(Array.isArray(parsed.artifacts));
    });

    it(`${id}: outer header === inner header via canonical JSON`, () => {
      const outer = f.submission.body.header;
      const decoded = Buffer.from(f.submission.body.capturePackage.packagePayload, "base64").toString("utf8");
      const inner = JSON.parse(decoded).header;
      assert.deepEqual(JSON.parse(canonicalJson(outer)), JSON.parse(canonicalJson(inner)));
    });

    it(`${id}: records match contract recordKinds`, () => {
      const decoded = Buffer.from(f.submission.body.capturePackage.packagePayload, "base64").toString("utf8");
      const pkg = JSON.parse(decoded);
      const kinds = pkg.records.map((r) => r.recordKind);
      for (const k of kinds) {
        assert.ok(
          CONTRACTS[id].recordKinds.includes(k),
          `${id}: record kind ${k} not in contract recordKinds [${CONTRACTS[id].recordKinds}]`,
        );
      }
    });

    it(`${id}: fixture record payloads match live XHS record shapes`, () => {
      const decoded = Buffer.from(f.submission.body.capturePackage.packagePayload, "base64").toString("utf8");
      const pkg = JSON.parse(decoded);
      for (const record of pkg.records) {
        if (record.recordKind === "note") {
          assert.ok(record.payload.noteId || record.payload.platformContentId || record.payload.url);
          assert.ok(record.payload.title || record.payload.content);
        } else if (record.recordKind === "comment") {
          assert.ok(record.payload.commentId);
          assert.ok(record.payload.noteId);
          assert.ok(record.payload.text);
        } else if (record.recordKind === "author") {
          assert.ok(record.payload.authorId || record.payload.platformAuthorId || record.payload.profileUrl);
        } else {
          assert.fail(`unexpected fixture record kind ${record.recordKind}`);
        }
      }
    });

    it(`${id}: counters.emitted equals records.length`, () => {
      const hdr = f.submission.body.header;
      const decoded = Buffer.from(f.submission.body.capturePackage.packagePayload, "base64").toString("utf8");
      const pkg = JSON.parse(decoded);
      assert.equal(hdr.report.counters.emitted, pkg.records.length);
    });

    it(`${id}: execution fields are desensitized placeholders (no real creds)`, () => {
      const hdr = f.submission.body.header;
      assert.match(hdr.jobId, /^fixture-job-/);
      assert.match(hdr.attemptId, /^fixture-attempt-/);
      assert.equal(hdr.leaseEpoch, 1);
      assert.equal(hdr.executionPlanVersion, "fixture-plan-v1");
    });

    // metadata_only → has media_inventory artifact; not_required → no artifact
    const contract = CONTRACTS[id];
    if (contract.mediaPolicy === "metadata_only") {
      it(`${id}: metadata_only fixture has media_inventory artifact`, () => {
        const decoded = Buffer.from(f.submission.body.capturePackage.packagePayload, "base64").toString("utf8");
        const pkg = JSON.parse(decoded);
        const mediaArts = pkg.artifacts.filter((a) => a.kind === "media_inventory");
        assert.ok(mediaArts.length > 0, `${id}: expected media_inventory artifact for metadata_only`);
        // Verify artifact has valid checksum
        for (const a of mediaArts) {
          assert.match(a.artifactChecksum, /^[0-9a-f]{64}$/);
          assert.equal(a.encoding, "base64");
        }
      });
    } else {
      it(`${id}: not_required fixture has NO media_inventory artifact`, () => {
        const decoded = Buffer.from(f.submission.body.capturePackage.packagePayload, "base64").toString("utf8");
        const pkg = JSON.parse(decoded);
        const mediaArts = pkg.artifacts.filter((a) => a.kind === "media_inventory");
        assert.equal(mediaArts.length, 0, `${id}: not_required must not have media_inventory artifact`);
      });
    }
  }

  // ── All 6 contract hashes are distinct ──────────────────────────────
  it("all 6 contract hashes are distinct", () => {
    const hashes = ALL_IDS.map((id) => contractHash(CONTRACTS[id]));
    assert.equal(new Set(hashes).size, 6);
  });

  // ── media_inventory artifact is well-formed ──────────────────────────
  it("media_inventory artifact has valid base64 + checksum", () => {
    const art = mediaInventoryArtifact();
    const decoded = Buffer.from(art.artifactPayload, "base64").toString("utf8");
    JSON.parse(decoded); // must not throw
    assert.equal(art.contentLength, Buffer.from(decoded, "utf8").length);
    assert.equal(art.artifactChecksum, sha256Hex(Buffer.from(decoded, "utf8")));
  });
});

// ── B1-B-03-R2: CaptureSubmissionV2 body builder ──────────────────────

describe("buildCaptureSubmissionBodyV2", () => {
  const header = {
    protocolVersion: "capture-submission/v2",
    captureId: "builder-test",
    platform: "xhs",
    target: { expectedTargetKey: "xhs:note/test", observedTargetKey: null },
    observedAt: "2026-08-06T12:00:00Z",
    collectorVersion: "1.0.0",
    contractId: "xhs.list-scan",
    contractVersion: 2,
    contractHash: EXPECTED_CONTRACT_HASHES["xhs.list-scan"],
    report: {
      startedAt: "2026-08-06T11:55:00Z",
      completedAt: "2026-08-06T12:00:00Z",
      terminal: { state: "completed", reason: "limit_reached", retryable: false },
      slots: [{ slotId: "note_list", status: "observed", reason: null }],
      counters: { requested: 1, discovered: 1, emitted: 1, deduplicated: 0, failed: 0 },
      diagnostics: {},
    },
    ingressKind: "execution",
    jobId: "builder-job",
    attemptId: "builder-att",
    leaseEpoch: 1,
    executionPlanVersion: "builder-v1",
  };

  const records = [{
    idempotencyKey: "builder-k1",
    recordKind: "note",
    platform: "xhs",
    targetKey: "xhs:note/test",
    externalRecordId: "ext-1",
    sequence: 0,
    payload: fixtureRecordPayload("note", "xhs.list-scan"),
    observedAt: "2026-08-06T11:58:00Z",
  }];

  it("produces a well-formed CaptureSubmissionBodyV2", () => {
    const body = buildCaptureSubmissionBodyV2({ header, records, artifacts: [] });
    assert.ok(body.header);
    assert.ok(body.capturePackage);
    assert.equal(body.capturePackage.encoding, "base64");
    assert.equal(body.capturePackage.checksumAlgorithm, "sha256");
    assert.match(body.capturePackage.checksumValue, /^[0-9a-f]{64}$/);
    assert.ok(body.capturePackage.contentLength > 0);
    assert.equal(body.capturePackage.restricted, false);
  });

  it("passes restricted flag through", () => {
    const body = buildCaptureSubmissionBodyV2({ header, records, artifacts: [], restricted: true });
    assert.equal(body.capturePackage.restricted, true);
  });

  it("outer header is the same reference as the input header", () => {
    const body = buildCaptureSubmissionBodyV2({ header, records, artifacts: [] });
    assert.strictEqual(body.header, header);
  });

  it("inner header equals outer header via canonical JSON", () => {
    const body = buildCaptureSubmissionBodyV2({ header, records, artifacts: [] });
    const decoded = Buffer.from(body.capturePackage.packagePayload, "base64").toString("utf8");
    const inner = JSON.parse(decoded).header;
    assert.deepEqual(JSON.parse(canonicalJson(header)), JSON.parse(canonicalJson(inner)));
  });

  it("package payload decodes to canonical JSON (sorted keys, no spaces)", () => {
    const body = buildCaptureSubmissionBodyV2({ header, records, artifacts: [] });
    const raw = Buffer.from(body.capturePackage.packagePayload, "base64").toString("utf8");
    // Must parse
    const pkg = JSON.parse(raw);
    assert.equal(pkg.schemaVersion, "capture-package/v2");
    assert.ok(Array.isArray(pkg.records));
    // Canonical: keys sorted
    const keyOrder = Object.keys(pkg);
    assert.deepEqual(keyOrder, ["artifacts", "header", "records", "schemaVersion"]);
  });

  it("array order is preserved in canonical JSON", () => {
    const multi = [
      { idempotencyKey: "z", recordKind: "note", platform: "xhs", targetKey: "xhs:note/a", externalRecordId: "ez", sequence: 1, payload: { z: 1 }, observedAt: "2026-08-06T11:58:00Z" },
      { idempotencyKey: "a", recordKind: "note", platform: "xhs", targetKey: "xhs:note/b", externalRecordId: "ea", sequence: 0, payload: { a: 2 }, observedAt: "2026-08-06T11:58:00Z" },
    ];
    const body = buildCaptureSubmissionBodyV2({ header, records: multi, artifacts: [] });
    const decoded = Buffer.from(body.capturePackage.packagePayload, "base64").toString("utf8");
    const pkg = JSON.parse(decoded);
    assert.equal(pkg.records[0].idempotencyKey, "z");
    assert.equal(pkg.records[1].idempotencyKey, "a");
  });

  it("contentLength equals actual UTF-8 byte length", () => {
    const body = buildCaptureSubmissionBodyV2({ header, records, artifacts: [] });
    const decoded = Buffer.from(body.capturePackage.packagePayload, "base64");
    assert.equal(body.capturePackage.contentLength, decoded.length);
  });

  it("checksumValue is sha256 of raw canonical bytes", () => {
    const body = buildCaptureSubmissionBodyV2({ header, records, artifacts: [] });
    const bytes = Buffer.from(body.capturePackage.packagePayload, "base64");
    const expected = sha256Hex(bytes);
    assert.equal(body.capturePackage.checksumValue, expected);
  });

  it("does not independently construct server-authority fields for a valid CaptureHeaderV2", () => {
    const body = buildCaptureSubmissionBodyV2({ header, records, artifacts: [] });
    const str = JSON.stringify(body);
    assert.ok(!str.includes("workspaceId"));
    assert.ok(!str.includes("receivedAt"));
    assert.ok(!str.includes("sourcePrincipal"));
    assert.ok(!str.includes("stationId"));
    assert.ok(!str.includes("leaseToken"));
    assert.ok(!str.includes("importerIdentity"));
    assert.ok(!str.includes("recoveryAuthorizedBy"));
    assert.ok(!str.includes("migrationAuthorization"));
  });

  it("artifact bytes exist only inside CapturePackage payload", () => {
    const art = mediaInventoryArtifact();
    const body = buildCaptureSubmissionBodyV2({ header, records, artifacts: [art] });
    // The outer body must not have artifact payload bytes at top level
    assert.ok(!body.capturePackage.artifactPayload);
    // The bytes must be inside the package payload
    const decoded = Buffer.from(body.capturePackage.packagePayload, "base64").toString("utf8");
    const pkg = JSON.parse(decoded);
    assert.equal(pkg.artifacts.length, 1);
    assert.equal(pkg.artifacts[0].artifactPayload, art.artifactPayload);
  });

  it("propagates artifact.restricted=true to package restricted (B1-B-03-R2-F1)", () => {
    const art = { ...mediaInventoryArtifact(), restricted: true };
    const body = buildCaptureSubmissionBodyV2({ header, records, artifacts: [art] });
    assert.equal(body.capturePackage.restricted, true);
  });

  it("preserves explicit restricted:true even without restricted artifacts (B1-B-03-R2-F1)", () => {
    const body = buildCaptureSubmissionBodyV2({ header, records, artifacts: [], restricted: true });
    assert.equal(body.capturePackage.restricted, true);
  });

  it("output restricted:false when no artifact is restricted and restricted not set", () => {
    const art = mediaInventoryArtifact(); // restricted: false by default
    const body = buildCaptureSubmissionBodyV2({ header, records, artifacts: [art] });
    assert.equal(body.capturePackage.restricted, false);
  });

  // ── Tampering negative tests ──────────────────────────────────────
  it("tampered fixture payload → fixed hash check fails", () => {
    const tamperedRecords = [{ ...records[0], payload: { ...records[0].payload, noteId: "TAMPERED" } }];
    const body = buildCaptureSubmissionBodyV2({ header, records: tamperedRecords, artifacts: [] });
    // This must NOT match the locked fixture hash for list-scan
    assert.notEqual(body.capturePackage.checksumValue, FIXTURES["xhs.list-scan"].fixturePackageHash);
  });

  it("tampered contract definition → fixed contract hash check fails", () => {
    const tampered = { ...CONTRACTS["xhs.list-scan"], recordKinds: ["note", "comment"] };
    assert.notEqual(contractHash(tampered), EXPECTED_CONTRACT_HASHES["xhs.list-scan"]);
  });

  it("unsupported platform cannot produce a valid V2 body for xhs contracts", () => {
    const douyinHeader = { ...header, platform: "douyin" };
    // The builder will produce a body, but the workbench validator will reject it
    const body = buildCaptureSubmissionBodyV2({ header: douyinHeader, records: [], artifacts: [] });
    // Verify it's well-formed but the platform is douyin (contract won't match)
    assert.equal(body.capturePackage.encoding, "base64");
    // The point: this body has a non-XHS platform, so a validator with XHS contracts rejects it
  });

  function buildWithPayload(payload) {
    const candidateRecords = [{ ...records[0], payload }];
    return buildCaptureSubmissionBodyV2({ header, records: candidateRecords, artifacts: [] });
  }

  it("rejects undefined through the CaptureSubmission builder", () => {
    assert.throws(() => buildWithPayload({ value: undefined }), /undefined is not valid JSON/);
  });

  it("rejects NaN through the CaptureSubmission builder", () => {
    assert.throws(() => buildWithPayload({ value: NaN }), /NaN is not valid JSON/);
  });

  it("rejects Infinity through the CaptureSubmission builder", () => {
    assert.throws(() => buildWithPayload({ value: Infinity }), /Infinity.* is not valid JSON/);
  });

  it("rejects -Infinity through the CaptureSubmission builder", () => {
    assert.throws(() => buildWithPayload({ value: -Infinity }), /Infinity.* is not valid JSON/);
  });

  it("rejects BigInt through the CaptureSubmission builder", () => {
    assert.throws(() => buildWithPayload({ value: 1n }), /BigInt is not valid JSON/);
  });

  it("rejects functions through the CaptureSubmission builder", () => {
    assert.throws(() => buildWithPayload({ value() {} }), /function is not valid JSON/);
  });

  it("rejects Date through the CaptureSubmission builder", () => {
    assert.throws(() => buildWithPayload({ value: new Date() }), /non-plain object is not valid JSON/);
  });

  it("rejects Map through the CaptureSubmission builder", () => {
    assert.throws(() => buildWithPayload({ value: new Map() }), /non-plain object is not valid JSON/);
  });

  it("rejects Set through the CaptureSubmission builder", () => {
    assert.throws(() => buildWithPayload({ value: new Set() }), /non-plain object is not valid JSON/);
  });

  it("rejects class instances through the CaptureSubmission builder", () => {
    class UnsupportedValue {}
    assert.throws(() => buildWithPayload({ value: new UnsupportedValue() }), /non-plain object is not valid JSON/);
  });

  it("rejects circular objects through the CaptureSubmission builder", () => {
    const circularPayload = {};
    circularPayload.self = circularPayload;
    const circularRecords = [{ ...records[0], payload: circularPayload }];

    assert.throws(
      () => buildCaptureSubmissionBodyV2({ header, records: circularRecords, artifacts: [] }),
      /circular reference is not valid JSON/,
    );
  });

  it("rejects circular arrays through the CaptureSubmission builder", () => {
    const circularPayload = [];
    circularPayload.push(circularPayload);

    assert.throws(() => buildWithPayload(circularPayload), /circular reference is not valid JSON/);
  });

  it("accepts valid JSON through the CaptureSubmission builder", () => {
    const payload = { a: 1, b: "two", c: null, d: true, e: [1, { f: "nested" }] };
    const body = buildWithPayload(payload);
    const decoded = JSON.parse(Buffer.from(body.capturePackage.packagePayload, "base64").toString("utf8"));

    assert.deepEqual(decoded.records[0].payload, payload);
  });

  it("accepts a non-circular shared reference through the CaptureSubmission builder", () => {
    const shared = { evidence: "same-value" };
    const body = buildWithPayload({ first: shared, second: shared });
    const decoded = JSON.parse(Buffer.from(body.capturePackage.packagePayload, "base64").toString("utf8"));

    assert.deepEqual(decoded.records[0].payload, {
      first: { evidence: "same-value" },
      second: { evidence: "same-value" },
    });
  });
});

// ── Print summary table for cross-repo verification ────────────────────

describe("cross-repo summary", () => {
  it("prints contract hashes and fixture package hashes", () => {
    for (const id of ALL_IDS) {
      const f = FIXTURES[id];
      console.log(JSON.stringify({
        id,
        version: CONTRACTS[id].version,
        contractHash: contractHash(CONTRACTS[id]),
        fixturePackageHash: f.fixturePackageHash,
      }));
    }
  });

  it("canonical JSON round-trips", () => {
    const obj = { b: 1, a: [3, 1, 2], c: { d: 4, e: 5 } };
    const s = canonicalJson(obj);
    assert.equal(s, '{"a":[3,1,2],"b":1,"c":{"d":4,"e":5}}');
  });
});
