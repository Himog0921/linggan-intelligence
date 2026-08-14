/**
 * Versioned XHS source contracts shared by the V2 terminal mapper and
 * mirrored by content-workbench. These contracts describe source facts only;
 * they do not create Domain or Projection records.
 */

const XHS_RECORD_PAYLOAD_CONTRACT = Object.freeze({
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
});

const XHS_MEDIA_INVENTORY_CONTRACT = Object.freeze({
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
});

const MEDIA_PURPOSES = new Set(XHS_MEDIA_INVENTORY_CONTRACT.purposes);
const MEDIA_KINDS = new Set(XHS_MEDIA_INVENTORY_CONTRACT.kinds);
const COVER_PROVENANCE = new Set(XHS_MEDIA_INVENTORY_CONTRACT.coverProvenance);

function isPlainObject(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

function nonEmptyString(value) {
  return typeof value === "string" && value.trim() === value && value.length > 0;
}

function exactKeys(value, keys) {
  if (!isPlainObject(value)) return false;
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function invalid(reason, path) {
  return { ok: false, reason, path };
}

function validateXhsRecordPayload(recordKind, payload) {
  if (!isPlainObject(payload)) return invalid("payload_not_object", "payload");

  if (recordKind === "note") {
    if (!nonEmptyString(payload.noteId)) return invalid("identity_missing", "payload.noteId");
    if (!nonEmptyString(payload.platformContentId)) {
      return invalid("identity_missing", "payload.platformContentId");
    }
    if (payload.noteId !== payload.platformContentId) {
      return invalid("identity_mismatch", "payload.noteId,platformContentId");
    }
    if (!nonEmptyString(payload.type)) return invalid("content_type_missing", "payload.type");
    if (!XHS_RECORD_PAYLOAD_CONTRACT.recordKinds.note.contentType.allowedValues.includes(payload.type)) {
      return invalid("content_type_invalid", "payload.type");
    }
    return { ok: true, identity: payload.noteId, contentType: payload.type };
  }

  if (recordKind === "comment") {
    if (!nonEmptyString(payload.noteId)) return invalid("identity_missing", "payload.noteId");
    if (!nonEmptyString(payload.commentId)) return invalid("identity_missing", "payload.commentId");
    return { ok: true, identity: `${payload.noteId}:${payload.commentId}` };
  }

  if (recordKind === "author") {
    if (!nonEmptyString(payload.authorId)) return invalid("identity_missing", "payload.authorId");
    if (!nonEmptyString(payload.platformAuthorId)) {
      return invalid("identity_missing", "payload.platformAuthorId");
    }
    if (payload.authorId !== payload.platformAuthorId) {
      return invalid("identity_mismatch", "payload.authorId,platformAuthorId");
    }
    return { ok: true, identity: payload.authorId };
  }

  return invalid("record_kind_unsupported", "recordKind");
}

function validateSubject(subject) {
  if (!isPlainObject(subject) || !nonEmptyString(subject.kind)) {
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

function validateXhsMediaInventoryV2(payload) {
  if (!exactKeys(payload, ["schemaVersion", "candidates"])) {
    return invalid("media_inventory_shape_invalid", "artifact");
  }
  if (payload.schemaVersion !== XHS_MEDIA_INVENTORY_CONTRACT.schemaVersion) {
    return invalid("media_inventory_version_invalid", "artifact.schemaVersion");
  }
  if (!Array.isArray(payload.candidates)) {
    return invalid("media_inventory_shape_invalid", "artifact.candidates");
  }

  const slots = new Set();
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
    if (!MEDIA_PURPOSES.has(candidate.purpose)) return invalid("media_purpose_invalid", `${path}.purpose`);
    if (!MEDIA_KINDS.has(candidate.kind)) return invalid("media_kind_invalid", `${path}.kind`);
    if (!Number.isSafeInteger(candidate.ordinal) || candidate.ordinal < 0) {
      return invalid("media_ordinal_invalid", `${path}.ordinal`);
    }
    if (!nonEmptyString(candidate.observedAddress)) {
      return invalid("media_address_invalid", `${path}.observedAddress`);
    }
    const expectedSlotId = `${subjectKey(candidate.subject)}:${candidate.purpose}:${candidate.kind}:${candidate.ordinal}`;
    if (candidate.slotId !== expectedSlotId) {
      return invalid("media_slot_mismatch", `${path}.slotId`);
    }
    if (!COVER_PROVENANCE.has(candidate.coverProvenance)) {
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
  return { ok: true, value: payload };
}

function subjectKey(subject) {
  return `note:${subject.noteId}`;
}

function validateXhsMediaInventorySubjects(payload, records) {
  const shape = validateXhsMediaInventoryV2(payload);
  if (!shape.ok) return shape;
  if (!Array.isArray(records)) return invalid("media_subject_mismatch", "records");
  const allowed = new Set();
  for (const record of records) {
    if (!isPlainObject(record) || !nonEmptyString(record.recordKind)) continue;
    const source = isPlainObject(record.payload) ? record.payload : record;
    const valid = validateXhsRecordPayload(record.recordKind, source);
    if (!valid.ok) continue;
    if (record.recordKind === "note") {
      allowed.add(subjectKey({ kind: "note", noteId: source.noteId, platformContentId: source.platformContentId }));
    }
  }
  for (let index = 0; index < payload.candidates.length; index += 1) {
    if (!allowed.has(subjectKey(payload.candidates[index].subject))) {
      return invalid("media_subject_mismatch", `artifact.candidates[${index}].subject`);
    }
  }
  return { ok: true, value: payload };
}

function purposeForAsset(asset) {
  const role = nonEmptyString(asset.role) ? asset.role : "";
  const kind = nonEmptyString(asset.assetType) ? asset.assetType : "";
  if (role === "cover") return "cover";
  if (kind === "video") return "video";
  if (kind === "live_photo") return "live_photo";
  if (role === "body" && kind === "image") return "body";
  return null;
}

function recordSubjects(records) {
  const byNote = new Map();
  for (const note of records.notes || []) {
    const valid = validateXhsRecordPayload("note", note);
    if (valid.ok) byNote.set(note.noteId, { kind: "note", noteId: note.noteId, platformContentId: note.platformContentId });
  }
  return { byNote };
}

function buildXhsMediaInventoryV2(records) {
  if (!isPlainObject(records) || !Array.isArray(records.mediaAssets)) {
    throw Object.assign(new Error("terminal mediaAssets must be an array"), { reason: "media_inventory_source_invalid" });
  }
  const subjects = recordSubjects(records);
  const candidates = [];

  for (const asset of records.mediaAssets) {
    if (!isPlainObject(asset)) {
      throw Object.assign(new Error("media asset must be an object"), { reason: "media_inventory_source_invalid" });
    }
    let subject = null;
    if (nonEmptyString(asset.noteId) && !nonEmptyString(asset.commentId)) {
      subject = subjects.byNote.get(asset.noteId) || null;
    }
    if (!subject) {
      throw Object.assign(new Error("media asset subject is not present in emitted records"), { reason: "media_subject_mismatch" });
    }

    const purpose = purposeForAsset(asset);
    const kind = nonEmptyString(asset.assetType) && MEDIA_KINDS.has(asset.assetType) ? asset.assetType : null;
    const observedAddress = nonEmptyString(asset.sourceUrl)
      ? asset.sourceUrl
      : (Array.isArray(asset.candidateUrls) && asset.candidateUrls.find(nonEmptyString)) || null;
    if (!purpose || !kind || !observedAddress || !Number.isSafeInteger(asset.ordinal) || asset.ordinal < 0) {
      throw Object.assign(new Error("media asset kind, purpose, ordinal, and observed address must be explicit"), { reason: "media_candidate_source_invalid" });
    }

    const group = `${subjectKey(subject)}:${purpose}:${kind}`;
    const ordinal = asset.ordinal;
    const coverProvenance = purpose === "cover" ? asset.coverProvenance : "not_cover";
    const candidate = {
      subject,
      slotId: `${group}:${ordinal}`,
      purpose,
      kind,
      ordinal,
      observedAddress,
      coverProvenance,
    };
    candidates.push(candidate);
  }

  candidates.sort((left, right) => left.slotId.localeCompare(right.slotId));
  const payload = { schemaVersion: XHS_MEDIA_INVENTORY_CONTRACT.schemaVersion, candidates };
  const valid = validateXhsMediaInventoryV2(payload);
  if (!valid.ok) {
    throw Object.assign(new Error(`${valid.path}: ${valid.reason}`), { reason: valid.reason });
  }
  return payload;
}

module.exports = {
  XHS_RECORD_PAYLOAD_CONTRACT,
  XHS_MEDIA_INVENTORY_CONTRACT,
  validateXhsRecordPayload,
  validateXhsMediaInventoryV2,
  validateXhsMediaInventorySubjects,
  buildXhsMediaInventoryV2,
};
