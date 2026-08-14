import type { JsonValue } from "../ingress/canonical-json";

export const XHS_RECORD_PAYLOAD_CONTRACT = {
  schemaVersion: "xhs.record-payload/v2",
  platform: "xhs",
  recordKinds: {
    note: {
      identityFields: ["noteId", "platformContentId"],
      identityRule: "non_empty_equal",
      contentType: { sourceField: "type", allowedValues: ["normal", "video"] },
    },
    comment: {
      identityFields: ["noteId", "commentId"],
      identityRule: "non_empty_pair",
    },
    author: {
      identityFields: ["authorId", "platformAuthorId"],
      identityRule: "non_empty_equal",
    },
  },
} as const;

export const XHS_MEDIA_INVENTORY_CONTRACT = {
  schemaVersion: "xhs.media-inventory/v2",
  platform: "xhs",
  candidateFields: [
    "subject",
    "slotId",
    "purpose",
    "kind",
    "ordinal",
    "observedAddress",
    "coverProvenance",
  ],
  subjectKinds: ["note"],
  slotFormula: "subjectKey:purpose:kind:ordinal",
  purposes: ["cover", "body", "video", "live_photo"],
  kinds: ["image", "video", "live_photo"],
  coverProvenance: ["platform_explicit", "first_observed_image", "not_cover"],
} as const;

export type XhsSourceRejectReason =
  | "payload_not_object"
  | "record_kind_unsupported"
  | "identity_missing"
  | "identity_mismatch"
  | "content_type_missing"
  | "content_type_invalid"
  | "media_inventory_shape_invalid"
  | "media_inventory_version_invalid"
  | "media_candidate_shape_invalid"
  | "media_subject_invalid"
  | "media_subject_mismatch"
  | "media_slot_invalid"
  | "media_slot_mismatch"
  | "media_slot_duplicate"
  | "media_purpose_invalid"
  | "media_kind_invalid"
  | "media_ordinal_invalid"
  | "media_address_invalid"
  | "media_cover_provenance_invalid";

export type XhsSourceValidation =
  | { ok: true }
  | { ok: false; reason: XhsSourceRejectReason; path: string };

function invalid(reason: XhsSourceRejectReason, path: string): XhsSourceValidation {
  return { ok: false, reason, path };
}

function isJsonObject(value: JsonValue): value is { [key: string]: JsonValue } {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function nonEmptyString(value: JsonValue | undefined): value is string {
  return typeof value === "string" && value.trim() === value && value.length > 0;
}

function exactKeys(value: JsonValue, expectedKeys: readonly string[]): value is { [key: string]: JsonValue } {
  if (!isJsonObject(value)) return false;
  const actual = Object.keys(value).sort();
  const expected = [...expectedKeys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

export function validateXhsRecordPayload(recordKind: string, payload: JsonValue): XhsSourceValidation {
  if (!isJsonObject(payload)) return invalid("payload_not_object", "payload");

  if (recordKind === "note") {
    if (!nonEmptyString(payload.noteId)) return invalid("identity_missing", "payload.noteId");
    if (!nonEmptyString(payload.platformContentId)) {
      return invalid("identity_missing", "payload.platformContentId");
    }
    if (payload.noteId !== payload.platformContentId) {
      return invalid("identity_mismatch", "payload.noteId,platformContentId");
    }
    if (!nonEmptyString(payload.type)) return invalid("content_type_missing", "payload.type");
    if (!(XHS_RECORD_PAYLOAD_CONTRACT.recordKinds.note.contentType.allowedValues as readonly string[])
      .includes(payload.type)) {
      return invalid("content_type_invalid", "payload.type");
    }
    return { ok: true };
  }

  if (recordKind === "comment") {
    if (!nonEmptyString(payload.noteId)) return invalid("identity_missing", "payload.noteId");
    if (!nonEmptyString(payload.commentId)) return invalid("identity_missing", "payload.commentId");
    return { ok: true };
  }

  if (recordKind === "author") {
    if (!nonEmptyString(payload.authorId)) return invalid("identity_missing", "payload.authorId");
    if (!nonEmptyString(payload.platformAuthorId)) {
      return invalid("identity_missing", "payload.platformAuthorId");
    }
    if (payload.authorId !== payload.platformAuthorId) {
      return invalid("identity_mismatch", "payload.authorId,platformAuthorId");
    }
    return { ok: true };
  }

  return invalid("record_kind_unsupported", "recordKind");
}

function validateSubject(subject: JsonValue): XhsSourceValidation {
  if (!isJsonObject(subject) || !nonEmptyString(subject.kind)) {
    return invalid("media_subject_invalid", "candidate.subject");
  }
  if (subject.kind === "note") {
    if (!exactKeys(subject, ["kind", "noteId", "platformContentId"])) {
      return invalid("media_subject_invalid", "candidate.subject");
    }
    if (!nonEmptyString(subject.noteId) || !nonEmptyString(subject.platformContentId)) {
      return invalid("identity_missing", "candidate.subject");
    }
    return subject.noteId === subject.platformContentId
      ? { ok: true }
      : invalid("identity_mismatch", "candidate.subject");
  }
  return invalid("media_subject_invalid", "candidate.subject.kind");
}

export function validateXhsMediaInventoryV2(payload: JsonValue): XhsSourceValidation {
  if (!exactKeys(payload, ["schemaVersion", "candidates"])) {
    return invalid("media_inventory_shape_invalid", "artifact");
  }
  if (payload.schemaVersion !== XHS_MEDIA_INVENTORY_CONTRACT.schemaVersion) {
    return invalid("media_inventory_version_invalid", "artifact.schemaVersion");
  }
  if (!Array.isArray(payload.candidates)) {
    return invalid("media_inventory_shape_invalid", "artifact.candidates");
  }

  const slots = new Set<string>();
  for (let index = 0; index < payload.candidates.length; index += 1) {
    const candidate = payload.candidates[index];
    const path = `artifact.candidates[${index}]`;
    if (!exactKeys(candidate, XHS_MEDIA_INVENTORY_CONTRACT.candidateFields)) {
      return invalid("media_candidate_shape_invalid", path);
    }
    const subject = validateSubject(candidate.subject);
    if (!subject.ok) return invalid(subject.reason, `${path}.subject`);
    if (!nonEmptyString(candidate.slotId)) return invalid("media_slot_invalid", `${path}.slotId`);
    if (slots.has(candidate.slotId)) return invalid("media_slot_duplicate", `${path}.slotId`);
    slots.add(candidate.slotId);
    if (!nonEmptyString(candidate.purpose) ||
        !(XHS_MEDIA_INVENTORY_CONTRACT.purposes as readonly string[]).includes(candidate.purpose)) {
      return invalid("media_purpose_invalid", `${path}.purpose`);
    }
    if (!nonEmptyString(candidate.kind) ||
        !(XHS_MEDIA_INVENTORY_CONTRACT.kinds as readonly string[]).includes(candidate.kind)) {
      return invalid("media_kind_invalid", `${path}.kind`);
    }
    if (!Number.isSafeInteger(candidate.ordinal) || (candidate.ordinal as number) < 0) {
      return invalid("media_ordinal_invalid", `${path}.ordinal`);
    }
    if (!nonEmptyString(candidate.observedAddress)) {
      return invalid("media_address_invalid", `${path}.observedAddress`);
    }
    const key = isJsonObject(candidate.subject) ? subjectKey(candidate.subject) : null;
    const expectedSlotId = `${key}:${candidate.purpose}:${candidate.kind}:${candidate.ordinal}`;
    if (!key || candidate.slotId !== expectedSlotId) {
      return invalid("media_slot_mismatch", `${path}.slotId`);
    }
    if (!nonEmptyString(candidate.coverProvenance) ||
        !(XHS_MEDIA_INVENTORY_CONTRACT.coverProvenance as readonly string[])
          .includes(candidate.coverProvenance)) {
      return invalid("media_cover_provenance_invalid", `${path}.coverProvenance`);
    }
    if (candidate.purpose === "cover") {
      if (candidate.kind !== "image" || candidate.coverProvenance === "not_cover") {
        return invalid("media_cover_provenance_invalid", `${path}.coverProvenance`);
      }
    } else if (candidate.coverProvenance !== "not_cover") {
      return invalid("media_cover_provenance_invalid", `${path}.coverProvenance`);
    }
  }
  return { ok: true };
}

function subjectKey(subject: { [key: string]: JsonValue }): string | null {
  if (subject.kind === "note" && nonEmptyString(subject.noteId)) return `note:${subject.noteId}`;
  return null;
}

export function validateXhsMediaInventorySubjects(
  payload: JsonValue,
  records: Array<{ recordKind: string; payload: JsonValue }>,
): XhsSourceValidation {
  const shape = validateXhsMediaInventoryV2(payload);
  if (!shape.ok || !isJsonObject(payload) || !Array.isArray(payload.candidates)) return shape;

  const allowed = new Set<string>();
  for (const record of records) {
    if (!isJsonObject(record.payload) || !validateXhsRecordPayload(record.recordKind, record.payload).ok) continue;
    const key = record.recordKind === "note"
      ? subjectKey({ kind: "note", noteId: record.payload.noteId, platformContentId: record.payload.platformContentId })
      : null;
    if (key) allowed.add(key);
  }

  for (let index = 0; index < payload.candidates.length; index += 1) {
    const candidate = payload.candidates[index];
    if (!isJsonObject(candidate) || !isJsonObject(candidate.subject) ||
        !allowed.has(subjectKey(candidate.subject) ?? "")) {
      return invalid("media_subject_mismatch", `artifact.candidates[${index}].subject`);
    }
  }
  return { ok: true };
}
