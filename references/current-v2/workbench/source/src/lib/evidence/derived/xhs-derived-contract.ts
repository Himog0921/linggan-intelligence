import {
  canonicalJsonBytes,
  canonicalJsonString,
  type JsonValue,
} from "../ingress/canonical-json";
import { sha256Hex } from "../ingress/base64";
import {
  validateXhsRecordPayload,
  type XhsSourceRejectReason,
} from "../contracts/xhs-source-contract";
import type {
  CaptureSlotV2,
  CaptureTerminalV2,
  CollectionContractDefinitionV2,
} from "../ingress/types";

export const XHS_ADAPTER_VERSION = "2.0.0";
export const XHS_CANONICAL_SCHEMA_VERSION = "xhs.canonical/2";
export const XHS_EVALUATOR_VERSION = "2.0.0";

/** Locked outputs from the plugin repo's six audited CapturePackage fixtures. */
export const XHS_DERIVED_FIXTURE_HASHES: Record<string, {
  normalizationOutputHashes: string[];
  evaluationInputHash: string;
}> = {
  "xhs.list-scan": {
    normalizationOutputHashes: ["4753151e8f66d985f133d2157d5cfcc19d21837b4e729c2dfb623aa422ff666e"],
    evaluationInputHash: "a036087f45727718ab88a9d01ee2afd3a23d6f7034df7437a8acddbdc767d85e",
  },
  "xhs.note-detail": {
    normalizationOutputHashes: [
      "1bcac6d7580e24cd219f5ba1847764ef0feae3ae8cc522f4d0e44335ff65d1fe",
      "2c4172db1f84465227bc32f0ecc92bb89146b50d08bc751daa1a4ad270f42eca",
    ],
    evaluationInputHash: "180008f73c1034ee5dcf33448e81f5a2671905f3c3b1d011e5cbda5c5351fbff",
  },
  "xhs.note-full": {
    normalizationOutputHashes: [
      "475011eeadc678440ee3d1021f937059a1dc37a29e90b35637a731489bb59838",
      "d37821e2cec35f567b59c213b62eaafcf1d88e4033962b9704e6bb86928210c6",
    ],
    evaluationInputHash: "86b16a890545dc4e40b27b2b35d0f7a399c6beae4511788fc5902c863595468c",
  },
  "xhs.comment-probe": {
    normalizationOutputHashes: ["47c8eeb96803047aaeb0945f4836f3613c6793cec87be303a771c94e4a1da012"],
    evaluationInputHash: "be49bf0d7cbd4032559a1c6ebf15249e8ebb48dae6aee33a5a306af737e5c858",
  },
  "xhs.author-profile": {
    normalizationOutputHashes: [
      "d7d2aed32cf64407e17bf702b2ab3c405f81f1583e9f9c7722709c7ab3d2dbb9",
      "4c3cd7d3f1c531945bfb902f6ade9e624d3983eecc8be1ad7919f5cd37e71259",
    ],
    evaluationInputHash: "dec1741c6e82e20f207e9d5e10553aaee6a8d79d727bc00aef56305d57dd9214",
  },
  "xhs.author-links": {
    normalizationOutputHashes: ["c747b6f154e0f752bda44f3a681282b2bfdcea84cf9dd8c2749445bd1033df63"],
    evaluationInputHash: "ff35a352a0f8f20b1e54782262508259d3a841b94361ef6556f64df62a5f4b6a",
  },
};

export type XhsRawRecordFact = {
  id: string;
  workspaceId: string;
  rawSnapshotId: string;
  recordKind: string | null;
  platform: string;
  sequence: number | null;
  payload: JsonValue;
  payloadHash: string | null;
  observedAt: Date;
};

type ParseError = {
  path: string;
  code: XhsSourceRejectReason
    | "canonicalization_failed"
    | "hash_mismatch"
    | "source_binding_mismatch";
};

export type XhsNormalizationResult = {
  adapterId: string;
  adapterVersion: typeof XHS_ADAPTER_VERSION;
  canonicalSchemaVersion: typeof XHS_CANONICAL_SCHEMA_VERSION;
  status: "normalized" | "rejected" | "quarantined";
  inputPayloadHash: string;
  outputPayloadHash: string | null;
  missingFields: string[] | null;
  parseErrors: ParseError[] | null;
  observation: {
    observationKind: "note" | "comment" | "author";
    subjectKey: string;
    observedAt: Date;
    schemaVersion: typeof XHS_CANONICAL_SCHEMA_VERSION;
    payload: JsonValue;
    payloadHash: string;
    qualityStatus: "complete" | "partial";
    fieldPresence: Record<string, boolean>;
  } | null;
};

export type XhsEvaluationMemberKind =
  | "current"
  | "current_version_mismatch"
  | "latest_rejected_attempt"
  | "latest_quarantined_attempt"
  | "normalized_current_missing"
  | "missing";

export type XhsEvaluationMember = {
  recordId: string;
  memberKind: XhsEvaluationMemberKind;
  runId: string | null;
  attemptNumber: number | null;
  status: "normalized" | "rejected" | "quarantined" | null;
  adapterId: string | null;
  adapterVersion: string | null;
  canonicalSchemaVersion: string | null;
  inputHash: string | null;
  outputHash: string | null;
  /** Sorting fact only; intentionally excluded from the frozen DTO. */
  sequence?: number | null;
};

export type XhsContractEvaluationResult = {
  evaluationInput: JsonValue;
  evaluationInputHash: string;
  decision: "accepted" | "rejected";
  completeness: "full" | "partial" | "not_applicable";
  rejectionCode:
    | "terminal_not_completed"
    | "required_slot_not_observed"
    | "required_slot_without_record"
    | "normalization_rejected"
    | "normalization_quarantined"
    | "normalization_current_missing"
    | "schema_version_mismatch"
    | "evaluation_input_mismatch"
    | null;
  rejectionReason: string | null;
  orderedMembers: XhsEvaluationMember[];
};

export function evaluateXhsContract(input: {
  snapshot: { workspaceId: string; rawSnapshotId: string };
  eligibility: {
    lifecycleStatus: "ACTIVE" | "ARCHIVED";
    integrityStatus: "verified";
  };
  contract: CollectionContractDefinitionV2;
  terminal: CaptureTerminalV2;
  slots: CaptureSlotV2[];
  members: XhsEvaluationMember[];
}): XhsContractEvaluationResult {
  const slots = input.slots
    .map((slot) => ({
      slotId: slot.slotId,
      status: slot.status,
      reason: slot.reason,
    }))
    .sort((left, right) => left.slotId.localeCompare(right.slotId));
  const orderedMembers = input.members
    .map((member) => ({ ...member }))
    .sort(compareEvaluationMembers);
  const serializedMembers = orderedMembers.map((member) => ({
    recordId: member.recordId,
    memberKind: member.memberKind,
    runId: member.runId,
    attemptNumber: member.attemptNumber,
    status: member.status,
    adapterId: member.adapterId,
    adapterVersion: member.adapterVersion,
    canonicalSchemaVersion: member.canonicalSchemaVersion,
    inputHash: member.inputHash,
    outputHash: member.outputHash,
  }));
  const evaluationInput: JsonValue = {
    snapshot: {
      workspaceId: input.snapshot.workspaceId,
      rawSnapshotId: input.snapshot.rawSnapshotId,
    },
    eligibility: {
      lifecycleStatus: input.eligibility.lifecycleStatus,
      integrityStatus: input.eligibility.integrityStatus,
    },
    contract: { id: input.contract.id, version: input.contract.version },
    terminal: {
      state: input.terminal.state,
      reason: input.terminal.reason,
      retryable: input.terminal.retryable,
    },
    slots,
    evaluator: {
      version: XHS_EVALUATOR_VERSION,
      canonicalSchemaVersion: XHS_CANONICAL_SCHEMA_VERSION,
    },
    members: serializedMembers,
  };
  const evaluationInputHash = sha256Hex(canonicalJsonBytes(evaluationInput));
  const rejection = firstRejection({
    terminal: input.terminal,
    contract: input.contract,
    slots,
    members: orderedMembers,
  });

  if (rejection !== null) {
    return {
      evaluationInput,
      evaluationInputHash,
      decision: "rejected",
      completeness: "not_applicable",
      rejectionCode: rejection.code,
      rejectionReason: canonicalJsonString(rejection),
      orderedMembers,
    };
  }

  const slotById = new Map(slots.map((slot) => [slot.slotId, slot]));
  const allDeclaredSlotsSatisfied = input.contract.slots.every((definition) => {
    const status = slotById.get(definition.slotId)?.status;
    return status === "observed" ||
      (definition.requirement !== "required" && status === "not_applicable");
  });

  return {
    evaluationInput,
    evaluationInputHash,
    decision: "accepted",
    completeness: allDeclaredSlotsSatisfied ? "full" : "partial",
    rejectionCode: null,
    rejectionReason: null,
    orderedMembers,
  };
}

type Rejection = {
  code: NonNullable<XhsContractEvaluationResult["rejectionCode"]>;
  details: JsonValue;
};

function firstRejection(input: {
  terminal: CaptureTerminalV2;
  contract: CollectionContractDefinitionV2;
  slots: CaptureSlotV2[];
  members: XhsEvaluationMember[];
}): Rejection | null {
  if (input.terminal.state !== "completed") {
    return {
      code: "terminal_not_completed",
      details: { state: input.terminal.state },
    };
  }

  const slotById = new Map(input.slots.map((slot) => [slot.slotId, slot]));
  const requiredNotObserved = input.contract.slots
    .filter((slot) => slot.requirement === "required")
    .filter((slot) => slotById.get(slot.slotId)?.status !== "observed")
    .map((slot) => slot.slotId)
    .sort();
  if (requiredNotObserved.length > 0) {
    return {
      code: "required_slot_not_observed",
      details: { slotIds: requiredNotObserved },
    };
  }

  const requiredWithoutRecord = input.contract.slots
    .filter((slot) => slot.requirement === "required")
    .filter((slot) => {
      const adapterId = adapterIdForSlot(slot.slotId);
      return adapterId === null || !input.members.some((member) => member.adapterId === adapterId);
    })
    .map((slot) => slot.slotId)
    .sort();
  if (requiredWithoutRecord.length > 0) {
    return {
      code: "required_slot_without_record",
      details: { slotIds: requiredWithoutRecord },
    };
  }

  const rejectedRecordIds = memberRecordIds(input.members, (member) =>
    member.memberKind === "latest_rejected_attempt" || member.status === "rejected");
  if (rejectedRecordIds.length > 0) {
    return {
      code: "normalization_rejected",
      details: { recordIds: rejectedRecordIds },
    };
  }

  const quarantinedRecordIds = memberRecordIds(input.members, (member) =>
    member.memberKind === "latest_quarantined_attempt" || member.status === "quarantined");
  if (quarantinedRecordIds.length > 0) {
    return {
      code: "normalization_quarantined",
      details: { recordIds: quarantinedRecordIds },
    };
  }

  const missingRecordIds = memberRecordIds(input.members, (member) =>
    member.memberKind === "missing" ||
    member.memberKind === "normalized_current_missing" ||
    member.status === null);
  if (missingRecordIds.length > 0) {
    return {
      code: "normalization_current_missing",
      details: { recordIds: missingRecordIds },
    };
  }

  const versionMismatchRecordIds = memberRecordIds(input.members, (member) =>
    member.memberKind === "current_version_mismatch" ||
    member.adapterVersion !== XHS_ADAPTER_VERSION ||
    member.canonicalSchemaVersion !== XHS_CANONICAL_SCHEMA_VERSION);
  if (versionMismatchRecordIds.length > 0) {
    return {
      code: "schema_version_mismatch",
      details: { recordIds: versionMismatchRecordIds },
    };
  }

  return null;
}

function adapterIdForSlot(slotId: string): string | null {
  switch (slotId) {
    case "note_list":
    case "note":
    case "note_links":
      return "xhs.note";
    case "comments":
      return "xhs.comment";
    case "author":
      return "xhs.author";
    default:
      return null;
  }
}

function memberRecordIds(
  members: XhsEvaluationMember[],
  predicate: (member: XhsEvaluationMember) => boolean,
): string[] {
  return members.filter(predicate).map((member) => member.recordId).sort();
}

function compareEvaluationMembers(left: XhsEvaluationMember, right: XhsEvaluationMember): number {
  const leftSequence = left.sequence;
  const rightSequence = right.sequence;
  if (leftSequence !== null && leftSequence !== undefined) {
    if (rightSequence === null || rightSequence === undefined) return -1;
    if (leftSequence !== rightSequence) return leftSequence - rightSequence;
  } else if (rightSequence !== null && rightSequence !== undefined) {
    return 1;
  }
  return left.recordId.localeCompare(right.recordId);
}

export function normalizeXhsRecord(record: XhsRawRecordFact): XhsNormalizationResult {
  const inputPayloadHash = sha256Hex(canonicalJsonBytes(record.payload));
  const adapter = resolveAdapter(record.recordKind);
  if (adapter === null) {
    return terminalResult("xhs.unsupported", inputPayloadHash, "rejected", null, [{
      path: "recordKind",
      code: "record_kind_unsupported",
    }]);
  }

  if (!isJsonObject(record.payload)) {
    return terminalResult(adapter.adapterId, inputPayloadHash, "rejected", null, [{
      path: "payload",
      code: "payload_not_object",
    }]);
  }
  if (record.platform !== "xhs" || !Number.isFinite(record.observedAt.getTime())) {
    return terminalResult(adapter.adapterId, inputPayloadHash, "quarantined", null, [{
      path: "record",
      code: "source_binding_mismatch",
    }]);
  }
  if (record.payloadHash !== null && record.payloadHash !== inputPayloadHash) {
    return terminalResult(adapter.adapterId, inputPayloadHash, "quarantined", null, [{
      path: "payload",
      code: "hash_mismatch",
    }]);
  }

  const sourceValidation = validateXhsRecordPayload(adapter.observationKind, record.payload);
  const identity = sourceValidation.ok ? adapter.identity(record.payload) : null;
  const fieldPresence = adapter.fieldPresence(record.payload, sourceValidation.ok);
  const missingFields = Object.entries(fieldPresence)
    .filter(([, present]) => !present)
    .map(([field]) => field)
    .sort();

  if (!sourceValidation.ok || identity === null) {
    return terminalResult(
      adapter.adapterId,
      inputPayloadHash,
      "rejected",
      missingFields,
      [{
        path: sourceValidation.ok ? adapter.identityPath(record.payload) : sourceValidation.path,
        code: sourceValidation.ok ? "identity_missing" : sourceValidation.reason,
      }],
    );
  }

  const subjectKey = identity.subjectKey;
  const payload: JsonValue = {
    platform: "xhs",
    recordKind: adapter.observationKind,
    subjectKey,
    observedAt: record.observedAt.toISOString(),
    sourcePayload: structuredClone(record.payload),
  };
  const outputPayloadHash = sha256Hex(canonicalJsonBytes(payload));

  return {
    adapterId: adapter.adapterId,
    adapterVersion: XHS_ADAPTER_VERSION,
    canonicalSchemaVersion: XHS_CANONICAL_SCHEMA_VERSION,
    status: "normalized",
    inputPayloadHash,
    outputPayloadHash,
    missingFields: missingFields.length === 0 ? null : missingFields,
    parseErrors: null,
    observation: {
      observationKind: adapter.observationKind,
      subjectKey,
      observedAt: new Date(record.observedAt.getTime()),
      schemaVersion: XHS_CANONICAL_SCHEMA_VERSION,
      payload,
      payloadHash: outputPayloadHash,
      qualityStatus: missingFields.length === 0 ? "complete" : "partial",
      fieldPresence,
    },
  };
}

type SupportedObservationKind = "note" | "comment" | "author";

type AdapterDefinition = {
  adapterId: `xhs.${SupportedObservationKind}`;
  observationKind: SupportedObservationKind;
  identity(payload: { [key: string]: JsonValue }): { subjectKey: string } | null;
  identityPath(payload: { [key: string]: JsonValue }): string;
  fieldPresence(
    payload: { [key: string]: JsonValue },
    identityPresent: boolean,
  ): Record<string, boolean>;
};

function resolveAdapter(recordKind: string | null): AdapterDefinition | null {
  switch (recordKind) {
    case "note":
      return {
        adapterId: "xhs.note",
        observationKind: "note",
        identity(payload) {
          const value = firstTrimmedString(payload, ["noteId"]);
          return value === null
            ? null
            : { subjectKey: `xhs:note:${encodeURIComponent(value)}` };
        },
        identityPath: () => "payload.noteIdentity",
        fieldPresence: (payload, identityPresent) => sortBooleanRecord({
          noteId: identityPresent && hasTrimmedString(payload.noteId),
          platformContentId: identityPresent && hasTrimmedString(payload.platformContentId),
          type: hasTrimmedString(payload.type),
          title: hasTrimmedString(payload.title),
          content: hasTrimmedString(payload.content),
          url: hasTrimmedString(payload.url),
        }),
      };
    case "comment":
      return {
        adapterId: "xhs.comment",
        observationKind: "comment",
        identity(payload) {
          const noteId = firstTrimmedString(payload, ["noteId"]);
          const commentId = firstTrimmedString(payload, ["commentId"]);
          return noteId === null || commentId === null
            ? null
            : {
                subjectKey: `xhs:comment:${encodeURIComponent(noteId)}:${encodeURIComponent(commentId)}`,
              };
        },
        identityPath(payload) {
          return firstTrimmedString(payload, ["noteId"]) === null
            ? "payload.noteId"
            : "payload.commentId";
        },
        fieldPresence: (payload) => sortBooleanRecord({
          noteId: hasTrimmedString(payload.noteId),
          commentId: hasTrimmedString(payload.commentId),
          text: hasTrimmedString(payload.text),
        }),
      };
    case "author":
      return {
        adapterId: "xhs.author",
        observationKind: "author",
        identity(payload) {
          const value = firstTrimmedString(payload, ["authorId"]);
          return value === null
            ? null
            : { subjectKey: `xhs:author:${encodeURIComponent(value)}` };
        },
        identityPath: () => "payload.authorIdentity",
        fieldPresence: (payload, identityPresent) => sortBooleanRecord({
          authorId: identityPresent && hasTrimmedString(payload.authorId),
          platformAuthorId: identityPresent && hasTrimmedString(payload.platformAuthorId),
          name: hasTrimmedString(payload.name),
          profileUrl: hasTrimmedString(payload.profileUrl),
        }),
      };
    default:
      return null;
  }
}

function terminalResult(
  adapterId: string,
  inputPayloadHash: string,
  status: "rejected" | "quarantined",
  missingFields: string[] | null,
  parseErrors: ParseError[],
): XhsNormalizationResult {
  return {
    adapterId,
    adapterVersion: XHS_ADAPTER_VERSION,
    canonicalSchemaVersion: XHS_CANONICAL_SCHEMA_VERSION,
    status,
    inputPayloadHash,
    outputPayloadHash: null,
    missingFields: missingFields && missingFields.length > 0 ? [...missingFields].sort() : null,
    parseErrors: [...parseErrors].sort(
      (left, right) => left.path.localeCompare(right.path) || left.code.localeCompare(right.code),
    ),
    observation: null,
  };
}

function isJsonObject(value: JsonValue): value is { [key: string]: JsonValue } {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function firstTrimmedString(
  payload: { [key: string]: JsonValue },
  keys: readonly string[],
): string | null {
  for (const key of keys) {
    const value = payload[key];
    if (hasTrimmedString(value)) return value.trim();
  }
  return null;
}

function hasTrimmedString(value: JsonValue | undefined): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function sortBooleanRecord(values: Record<string, boolean>): Record<string, boolean> {
  return Object.fromEntries(
    Object.entries(values).sort(([left], [right]) => left.localeCompare(right)),
  );
}
