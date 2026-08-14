/**
 * XHS V2 CollectionContract canonical definitions.
 *
 * B1-B-03-R1: plugin is the source of truth.  Workbench mirrors these same
 * definitions into the CollectionContractRegistry.  Any cross-repo difference
 * in canonical JSON, contract hash, or fixture hash is a hard failure.
 *
 * Source audit (08-xhs-collection-contracts.md §1):
 *   taskLeaseClient.js:345-350  → capability → contract mapping
 *   taskPoller.js:799-837       → record types actually produced
 *   MESSAGE_PROTOCOL.md:252-285  → terminal states
 */

const {
  XHS_RECORD_PAYLOAD_CONTRACT,
  XHS_MEDIA_INVENTORY_CONTRACT,
  validateXhsMediaInventoryV2,
} = require("./xhs-source-contract.cjs");

// ── Canonical JSON ─────────────────────────────────────────────────────────

/**
 * Validate that a value is legal JSON (07 §2.1).
 * Rejects: undefined, NaN, Infinity, BigInt, functions, non-plain objects
 * (Date, Map, Set, class instances), and circular references.
 * Must match workbench assertJsonValue() semantics.
 */
function assertJsonValue(value, path, ancestors) {
  path = path || "root";
  ancestors = ancestors || new Set();
  if (value === undefined) throw new Error(path + ": undefined is not valid JSON");
  if (typeof value === "number") {
    if (Number.isNaN(value)) throw new Error(path + ": NaN is not valid JSON");
    if (!Number.isFinite(value)) throw new Error(path + ": Infinity/-Infinity is not valid JSON");
    return;
  }
  if (typeof value === "bigint") throw new Error(path + ": BigInt is not valid JSON");
  if (typeof value === "function") throw new Error(path + ": function is not valid JSON");
  if (value === null || typeof value === "string" || typeof value === "boolean") return;
  if (Array.isArray(value)) {
    if (ancestors.has(value)) throw new Error(path + ": circular reference is not valid JSON");
    ancestors.add(value);
    try {
      for (let i = 0; i < value.length; i++) {
        assertJsonValue(value[i], path + "[" + i + "]", ancestors);
      }
    } finally {
      ancestors.delete(value);
    }
    return;
  }
  if (typeof value === "object") {
    const proto = Object.getPrototypeOf(value);
    if (proto !== Object.prototype && proto !== null) {
      throw new Error(path + ": non-plain object is not valid JSON");
    }
    if (ancestors.has(value)) throw new Error(path + ": circular reference is not valid JSON");
    ancestors.add(value);
    try {
      for (const key of Object.keys(value)) {
        assertJsonValue(value[key], path + "." + key, ancestors);
      }
    } finally {
      ancestors.delete(value);
    }
    return;
  }
  throw new Error(path + ": unexpected type " + typeof value);
}

/**
 * Check if a value is a plain object.
 */
function isPlainObject(value) {
  if (value === null || typeof value !== "object") return false;
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

/**
 * Recursively sort object keys. Returns a new value; does not mutate input.
 */
function sortKeys(value) {
  if (Array.isArray(value)) return value.map(sortKeys);
  if (isPlainObject(value)) {
    const sorted = {};
    for (const key of Object.keys(value).sort()) {
      sorted[key] = sortKeys(value[key]);
    }
    return sorted;
  }
  return value;
}

/**
 * Canonical JSON string: object keys sorted, arrays preserved, no whitespace.
 * Validates input for JSON-safe values before serializing.
 * Mirrors workbench src/lib/evidence/ingress/canonical-json.ts canonicalJsonString().
 */
function canonicalJson(value) {
  assertJsonValue(value);
  return JSON.stringify(sortKeys(value));
}

// ── SHA-256 ────────────────────────────────────────────────────────────────

const SHA256_K = Object.freeze([
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
]);

function rotateRight(value, count) {
  return (value >>> count) | (value << (32 - count));
}

function sha256Hex(value) {
  const source = typeof value === "string" ? new TextEncoder().encode(value) : value;
  if (!(source instanceof Uint8Array)) {
    throw new TypeError("sha256Hex accepts only UTF-8 text or Uint8Array bytes");
  }
  const paddedLength = Math.ceil((source.length + 9) / 64) * 64;
  const bytes = new Uint8Array(paddedLength);
  bytes.set(source);
  bytes[source.length] = 0x80;
  const bitLength = source.length * 8;
  const view = new DataView(bytes.buffer);
  view.setUint32(paddedLength - 8, Math.floor(bitLength / 0x100000000), false);
  view.setUint32(paddedLength - 4, bitLength >>> 0, false);

  const hash = new Uint32Array([
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
  ]);
  const words = new Uint32Array(64);
  for (let offset = 0; offset < paddedLength; offset += 64) {
    for (let index = 0; index < 16; index += 1) {
      words[index] = view.getUint32(offset + (index * 4), false);
    }
    for (let index = 16; index < 64; index += 1) {
      const x = words[index - 15];
      const y = words[index - 2];
      const sigma0 = rotateRight(x, 7) ^ rotateRight(x, 18) ^ (x >>> 3);
      const sigma1 = rotateRight(y, 17) ^ rotateRight(y, 19) ^ (y >>> 10);
      words[index] = (words[index - 16] + sigma0 + words[index - 7] + sigma1) >>> 0;
    }

    let [a, b, c, d, e, f, g, h] = hash;
    for (let index = 0; index < 64; index += 1) {
      const upper1 = rotateRight(e, 6) ^ rotateRight(e, 11) ^ rotateRight(e, 25);
      const choice = (e & f) ^ (~e & g);
      const temp1 = (h + upper1 + choice + SHA256_K[index] + words[index]) >>> 0;
      const upper0 = rotateRight(a, 2) ^ rotateRight(a, 13) ^ rotateRight(a, 22);
      const majority = (a & b) ^ (a & c) ^ (b & c);
      const temp2 = (upper0 + majority) >>> 0;
      h = g;
      g = f;
      f = e;
      e = (d + temp1) >>> 0;
      d = c;
      c = b;
      b = a;
      a = (temp1 + temp2) >>> 0;
    }
    hash[0] = (hash[0] + a) >>> 0;
    hash[1] = (hash[1] + b) >>> 0;
    hash[2] = (hash[2] + c) >>> 0;
    hash[3] = (hash[3] + d) >>> 0;
    hash[4] = (hash[4] + e) >>> 0;
    hash[5] = (hash[5] + f) >>> 0;
    hash[6] = (hash[6] + g) >>> 0;
    hash[7] = (hash[7] + h) >>> 0;
  }
  return Array.from(hash, (word) => word.toString(16).padStart(8, "0")).join("");
}

// ── Strict Base64 ──────────────────────────────────────────────────────────

const BASE64_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

function encodeBase64(bytes) {
  if (!(bytes instanceof Uint8Array)) {
    throw new TypeError("encodeBase64 accepts only Uint8Array bytes");
  }
  let encoded = "";
  for (let offset = 0; offset < bytes.length; offset += 3) {
    const first = bytes[offset];
    const hasSecond = offset + 1 < bytes.length;
    const hasThird = offset + 2 < bytes.length;
    const second = hasSecond ? bytes[offset + 1] : 0;
    const third = hasThird ? bytes[offset + 2] : 0;
    const value = (first << 16) | (second << 8) | third;
    encoded += BASE64_ALPHABET[(value >>> 18) & 63];
    encoded += BASE64_ALPHABET[(value >>> 12) & 63];
    encoded += hasSecond ? BASE64_ALPHABET[(value >>> 6) & 63] : "=";
    encoded += hasThird ? BASE64_ALPHABET[value & 63] : "=";
  }
  return encoded;
}

function utf8Bytes(value) {
  return new TextEncoder().encode(value);
}

// ── Contract hash ──────────────────────────────────────────────────────────

/**
 * Contract hash = sha256(UTF-8(canonicalJson(definition))).
 * Lowercase 64-char hex.  Mirrors workbench computeContractHash().
 */
function contractHash(definition) {
  return sha256Hex(canonicalJson(definition));
}

const XHS_SOURCE_CONTRACT_BINDINGS = Object.freeze({
  recordPayload: Object.freeze({
    schemaVersion: XHS_RECORD_PAYLOAD_CONTRACT.schemaVersion,
    contractHash: contractHash(XHS_RECORD_PAYLOAD_CONTRACT),
  }),
  mediaInventory: Object.freeze({
    schemaVersion: XHS_MEDIA_INVENTORY_CONTRACT.schemaVersion,
    contractHash: contractHash(XHS_MEDIA_INVENTORY_CONTRACT),
  }),
});

// ── Package hash ───────────────────────────────────────────────────────────

/**
 * Package hash = sha256(raw canonical JSON bytes).
 * Lowercase 64-char hex.
 */
function packageHash(canonicalPayloadJson) {
  return sha256Hex(canonicalPayloadJson);
}

// ── CaptureSubmissionV2 body builder (B1-B-03-R2) ──────────────────────────

/**
 * Build a canonical CaptureSubmissionBodyV2 from header, records, and artifacts.
 *
 * Per 07 §3.3 and DEC-B1-017, this function:
 *   1. Wraps header + records + artifacts in { schemaVersion: "capture-package/v2", … }
 *   2. Canonicalises object keys (sorted), preserves array order
 *   3. Produces no-space JSON → UTF-8 bytes
 *   4. Encodes with RFC 4648 standard padded base64
 *   5. Computes lowercase 64-char hex SHA-256 over the raw canonical bytes
 *   6. Sets contentLength to the actual UTF-8 byte length
 *   7. The same header object is placed both as the outer header and inside
 *      the CapturePackage payload — the caller must pass the same object.
 *
 * Given a valid CaptureHeaderV2 input, does not independently construct
 * server-authority fields. Those fields belong to the workbench caller adapter
 * boundary (07 §3.4); this builder forwards the caller-provided header unchanged.
 *
 * @param {Object} input
 * @param {Object} input.header       - CaptureHeaderV2 (used as both outer and inner header)
 * @param {Array}  input.records      - RawRecordSubmissionV2[]
 * @param {Array}  input.artifacts    - CaptureArtifactSubmissionV2[]
 * @param {boolean} [input.restricted=false] - package restricted flag (§3.3)
 * @returns {Object} CaptureSubmissionBodyV2 { header, capturePackage }
 */
function buildCaptureSubmissionBodyV2({ header, records, artifacts, restricted = false }) {
  // §3.3: if any artifact is restricted, the package MUST be restricted.
  // An explicit restricted: false with a restricted artifact would be rejected
  // by the workbench validator as invalid_submission.  We enforce the
  // propagation here so the caller never produces an invalid combination.
  const effectiveRestricted = restricted || (artifacts || []).some(a => a.restricted === true);
  const payload = {
    schemaVersion: "capture-package/v2",
    header,
    records,
    artifacts,
  };
  const json = canonicalJson(payload);
  const bytes = utf8Bytes(json);
  return {
    header,
    capturePackage: {
      encoding: "base64",
      packagePayload: encodeBase64(bytes),
      checksumAlgorithm: "sha256",
      checksumValue: packageHash(json),
      contentLength: bytes.length,
      restricted: effectiveRestricted,
    },
  };
}

// ── Legacy internal helper (kept for fixture hash stability) ───────────────

function buildPackageSubmission(header, records, artifacts) {
  return buildCaptureSubmissionBodyV2({ header, records, artifacts }).capturePackage;
}

// ── Common header fields ───────────────────────────────────────────────────

function commonHeader(contract) {
  return {
    protocolVersion: "capture-submission/v2",
    captureId: `v2-fixture-${contract.id}`,
    platform: "xhs",
    target: {
      expectedTargetKey: `xhs:fixture/${contract.id}`,
      observedTargetKey: `xhs:fixture/${contract.id}`,
    },
    observedAt: "2026-08-05T12:00:00Z",
    collectorVersion: "v2-fixture-1.0.0",
    contractId: contract.id,
    contractVersion: contract.version,
    contractHash: contractHash(contract),
    ingressKind: "execution",
    // Desensitized execution fields — no real credentials
    jobId: `fixture-job-${contract.id}`,
    attemptId: `fixture-attempt-${contract.id}`,
    leaseEpoch: 1,
    executionPlanVersion: "fixture-plan-v1",
  };
}

/**
 * Build report slots from contract slot definitions.
 * All slots are "observed" for a successful fixture.
 */
function reportSlots(slotIds) {
  return slotIds.map((slotId) => ({
    slotId,
    status: "observed",
    reason: null,
  }));
}

// ── Media inventory artifact (for metadata_only contracts) ─────────────────

function mediaInventoryArtifact(subject = {
  kind: "note",
  noteId: "fixture-note-xhs-list-scan",
  platformContentId: "fixture-note-xhs-list-scan",
}) {
  const payload = {
    schemaVersion: XHS_MEDIA_INVENTORY_CONTRACT.schemaVersion,
    candidates: [{
      subject,
      slotId: `note:${subject.noteId}:cover:image:0`,
      purpose: "cover",
      kind: "image",
      ordinal: 0,
      observedAddress: "https://sns-webpic-qc.xhscdn.com/fixture-cover",
      coverProvenance: "platform_explicit",
    }],
  };
  return mediaInventoryArtifactFromCandidates(payload.candidates);
}

/**
 * Build a media inventory artifact from the current collection run's actual
 * media records. The legacy no-argument fixture helper above remains separate
 * so its locked fixture hashes cannot accidentally become a runtime source.
 *
 * @param {Array<unknown>} candidates
 */
function mediaInventoryArtifactFromCandidates(candidates) {
  if (!Array.isArray(candidates)) {
    throw new Error("media inventory candidates must be an array");
  }
  const payload = {
    schemaVersion: XHS_MEDIA_INVENTORY_CONTRACT.schemaVersion,
    candidates,
  };
  const validation = validateXhsMediaInventoryV2(payload);
  if (!validation.ok) {
    throw new Error(`${validation.path}: ${validation.reason}`);
  }
  const json = canonicalJson(payload);
  const bytes = utf8Bytes(json);
  return {
    kind: "media_inventory",
    encoding: "base64",
    artifactPayload: encodeBase64(bytes),
    artifactChecksum: sha256Hex(bytes),
    contentLength: bytes.length,
    restricted: false,
  };
}

/**
 * Fixture payloads use the same stable identity/body fields that the live
 * taskPoller sanitizers and recordPayloadValidator accept.  These are
 * desensitized values, but they must still be source-shaped rather than a
 * made-up `{ title, content }` payload for every record kind.
 */
function fixtureRecordPayload(kind, contractId) {
  const suffix = contractId.replace(/[^a-z0-9-]/gi, "-");
  if (kind === "note") {
    const type = contractId === "xhs.note-detail" || contractId === "xhs.note-full"
      ? "video"
      : "normal";
    return {
      platform: "xhs",
      noteId: `fixture-note-${suffix}`,
      platformContentId: `fixture-note-${suffix}`,
      type,
      title: `Fixture note for ${contractId}`,
      content: `Desensitized note body for V2 contract ${contractId}`,
      url: `https://www.xiaohongshu.com/explore/fixture-${suffix}`,
    };
  }
  if (kind === "comment") {
    return {
      platform: "xhs",
      commentId: `fixture-comment-${suffix}`,
      noteId: `fixture-note-${suffix}`,
      text: `Desensitized comment for V2 contract ${contractId}`,
    };
  }
  if (kind === "author") {
    return {
      platform: "xhs",
      authorId: `fixture-author-${suffix}`,
      platformAuthorId: `fixture-author-${suffix}`,
      name: `Fixture author for ${contractId}`,
      profileUrl: `https://www.xiaohongshu.com/user/profile/fixture-${suffix}`,
    };
  }
  throw new Error(`Unsupported fixture record kind: ${kind}`);
}

// ── 6 CONTRACT DEFINITIONS (08 §2) ─────────────────────────────────────────

const CONTRACTS = {
  "xhs.list-scan": {
    id: "xhs.list-scan",
    version: 2,
    sourceContracts: XHS_SOURCE_CONTRACT_BINDINGS,
    platforms: ["xhs"],
    recordKinds: ["note"],
    slots: [{ slotId: "note_list", requirement: "required" }],
    terminalPolicy: {
      allowedStates: ["completed", "blocked", "cancelled", "error"],
      allowEmptyRecords: true,
    },
    mediaPolicy: "metadata_only",
  },

  "xhs.note-detail": {
    id: "xhs.note-detail",
    version: 2,
    sourceContracts: XHS_SOURCE_CONTRACT_BINDINGS,
    platforms: ["xhs"],
    recordKinds: ["note", "comment"],
    slots: [
      { slotId: "note", requirement: "required" },
      { slotId: "comments", requirement: "conditional" },
    ],
    terminalPolicy: {
      allowedStates: ["completed", "blocked", "cancelled", "error"],
      allowEmptyRecords: true,
    },
    mediaPolicy: "metadata_only",
  },

  "xhs.note-full": {
    id: "xhs.note-full",
    version: 2,
    sourceContracts: XHS_SOURCE_CONTRACT_BINDINGS,
    platforms: ["xhs"],
    recordKinds: ["note", "comment"],
    slots: [
      { slotId: "note", requirement: "required" },
      { slotId: "comments", requirement: "required" },
    ],
    terminalPolicy: {
      allowedStates: ["completed", "blocked", "cancelled", "error"],
      allowEmptyRecords: true,
    },
    mediaPolicy: "metadata_only",
  },

  "xhs.comment-probe": {
    id: "xhs.comment-probe",
    version: 2,
    sourceContracts: XHS_SOURCE_CONTRACT_BINDINGS,
    platforms: ["xhs"],
    recordKinds: ["comment"],
    slots: [{ slotId: "comments", requirement: "required" }],
    terminalPolicy: {
      allowedStates: ["completed", "blocked", "cancelled", "error"],
      allowEmptyRecords: true,
    },
    mediaPolicy: "not_required",
  },

  "xhs.author-profile": {
    id: "xhs.author-profile",
    version: 2,
    sourceContracts: XHS_SOURCE_CONTRACT_BINDINGS,
    platforms: ["xhs"],
    recordKinds: ["author", "note"],
    slots: [
      { slotId: "author", requirement: "required" },
      { slotId: "note_list", requirement: "optional" },
    ],
    terminalPolicy: {
      allowedStates: ["completed", "blocked", "cancelled", "error"],
      allowEmptyRecords: true,
    },
    mediaPolicy: "metadata_only",
  },

  "xhs.author-links": {
    id: "xhs.author-links",
    version: 2,
    sourceContracts: XHS_SOURCE_CONTRACT_BINDINGS,
    platforms: ["xhs"],
    recordKinds: ["note"],
    slots: [{ slotId: "note_links", requirement: "required" }],
    terminalPolicy: {
      allowedStates: ["completed", "blocked", "cancelled", "error"],
      allowEmptyRecords: true,
    },
    mediaPolicy: "not_required",
  },
};

// These fixture blueprints are deliberately independent from CONTRACTS. They
// are the six desensitized shapes audited against the live XHS taskPoller;
// changing a contract cannot silently regenerate a matching fixture.
const FIXTURE_BLUEPRINTS = {
  "xhs.list-scan": { recordKinds: ["note"], slotIds: ["note_list"], mediaPolicy: "metadata_only" },
  "xhs.note-detail": { recordKinds: ["note", "comment"], slotIds: ["note", "comments"], mediaPolicy: "metadata_only" },
  "xhs.note-full": { recordKinds: ["note", "comment"], slotIds: ["note", "comments"], mediaPolicy: "metadata_only" },
  "xhs.comment-probe": { recordKinds: ["comment"], slotIds: ["comments"], mediaPolicy: "not_required" },
  "xhs.author-profile": { recordKinds: ["author", "note"], slotIds: ["author", "note_list"], mediaPolicy: "metadata_only" },
  "xhs.author-links": { recordKinds: ["note"], slotIds: ["note_links"], mediaPolicy: "not_required" },
};

// Contract hashes are fixed audit anchors. A contract edit must update the
// audited fixture explicitly; it may not silently move both sides together.
const EXPECTED_CONTRACT_HASHES = {
  "xhs.list-scan": "9d3347aca8897dc3c373f72cc4047f20c33c990be29e94b1698b47cde153c37f",
  "xhs.note-detail": "044fd0cc82cb9fca81c3e3665c20b00af7bbeebc0657924fc01543910df65276",
  "xhs.note-full": "ec33bbdee4dbe4dc3e9598770e8d504c2eae9c2ae6103249c699dcf98d618f0c",
  "xhs.comment-probe": "f2f6408d9527ff169d52db8fc45e5faecee6ddc84b7ad027978df94d3e74200f",
  "xhs.author-profile": "d2b29fd9f74ad379eb41bc16ef20ba89311a4fa914f5f189cadd4b08714f0405",
  "xhs.author-links": "2d19d1ae951f3d1d2d66a24c0ee5037f06f2f87dee9c1418bf24aec791fd6e89",
};

// ── 6 fixture CapturePayloadV2's ───────────────────────────────────────────

function buildFixture(contractId) {
  const contract = CONTRACTS[contractId];
  if (!contract) throw new Error(`Unknown contract: ${contractId}`);
  const blueprint = FIXTURE_BLUEPRINTS[contractId];
  if (!blueprint) throw new Error(`Missing fixture blueprint: ${contractId}`);
  const fixtureContractHash = contractHash(contract);
  if (fixtureContractHash !== EXPECTED_CONTRACT_HASHES[contractId]) {
    throw new Error(`Fixture audit anchor mismatch: ${contractId}`);
  }

  const header = commonHeader(contract);
  header.report = {
    startedAt: "2026-08-05T11:55:00Z",
    completedAt: "2026-08-05T12:00:00Z",
    terminal: { state: "completed", reason: "limit_reached", retryable: false },
    slots: reportSlots(blueprint.slotIds),
    counters: { requested: 10, discovered: 5, emitted: 3, deduplicated: 2, failed: 0 },
    diagnostics: {},
  };

  const records = [];
  let seq = 0;

  for (const kind of blueprint.recordKinds) {
    records.push({
      idempotencyKey: `fixture-${contractId}-${kind}-001`,
      recordKind: kind,
      platform: "xhs",
      targetKey: `xhs:fixture/${contractId}`,
      externalRecordId: `ext-fixture-${contractId}-${kind}`,
      sequence: seq++,
      payload: fixtureRecordPayload(kind, contractId),
      observedAt: "2026-08-05T11:58:00Z",
    });
  }
  header.report.counters.emitted = records.length;

  // Artifacts: metadata_only contracts get media_inventory; not_required get none
  const fixtureNote = records.find((record) => record.recordKind === "note")?.payload;
  const artifacts = blueprint.mediaPolicy === "metadata_only" && fixtureNote
    ? [mediaInventoryArtifact({
        kind: "note",
        noteId: fixtureNote.noteId,
        platformContentId: fixtureNote.platformContentId,
      })]
    : [];

  const body = buildCaptureSubmissionBodyV2({ header, records, artifacts });

  const authority = {
    ingressKind: "execution",
    workspaceId: "ws-fixture",
    receivedAt: new Date("2026-08-05T12:00:01Z"),
    sourcePrincipal: "fixture-agent@b1-b03-r1",
    stationId: "station-fixture",
    leaseToken: "lease-token-fixture",
  };

  const submission = { body, authority };

  return {
    contractId,
    contract,
    contractHash: contractHash(contract),
    fixtureHeader: header,
    fixturePackageHash: body.capturePackage.checksumValue,
    submission,
  };
}

const FIXTURES = {};
for (const id of Object.keys(CONTRACTS)) {
  FIXTURES[id] = buildFixture(id);
}

// ── Exports ────────────────────────────────────────────────────────────────

module.exports = {
  XHS_SOURCE_CONTRACT_BINDINGS,
  canonicalJson,
  sha256Hex,
  encodeBase64,
  contractHash,
  packageHash,
  buildCaptureSubmissionBodyV2,
  buildPackageSubmission,
  mediaInventoryArtifact,
  mediaInventoryArtifactFromCandidates,
  fixtureRecordPayload,
  CONTRACTS,
  FIXTURE_BLUEPRINTS,
  EXPECTED_CONTRACT_HASHES,
  FIXTURES,
};
