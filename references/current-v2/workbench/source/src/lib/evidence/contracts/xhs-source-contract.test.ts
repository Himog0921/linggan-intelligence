// @vitest-environment node

import { describe, expect, it } from "vitest";

import {
  XHS_MEDIA_INVENTORY_CONTRACT,
  XHS_RECORD_PAYLOAD_CONTRACT,
  validateXhsMediaInventoryV2,
  validateXhsMediaInventorySubjects,
  validateXhsRecordPayload,
} from "./xhs-source-contract";

const VALID_CANDIDATE = {
  subject: { kind: "note", noteId: "note-1", platformContentId: "note-1" },
  slotId: "note:note-1:cover:image:0",
  purpose: "cover",
  kind: "image",
  ordinal: 0,
  observedAddress: "https://sns-webpic-qc.xhscdn.com/note-1-cover",
  coverProvenance: "platform_explicit",
} as const;

describe("XHS versioned source contracts", () => {
  it("locks the record and media schema versions", () => {
    expect(XHS_RECORD_PAYLOAD_CONTRACT.schemaVersion).toBe("xhs.record-payload/v2");
    expect(XHS_MEDIA_INVENTORY_CONTRACT.schemaVersion).toBe("xhs.media-inventory/v2");
  });

  it("accepts strict equal note and author identities plus normal/video", () => {
    expect(validateXhsRecordPayload("note", {
      noteId: "note-1",
      platformContentId: "note-1",
      type: "normal",
    })).toEqual({ ok: true });
    expect(validateXhsRecordPayload("note", {
      noteId: "note-2",
      platformContentId: "note-2",
      type: "video",
    })).toEqual({ ok: true });
    expect(validateXhsRecordPayload("author", {
      authorId: "author-1",
      platformAuthorId: "author-1",
    })).toEqual({ ok: true });
  });

  it("rejects identity mismatch, a legacy type alias, and unknown type", () => {
    expect(validateXhsRecordPayload("note", {
      noteId: "note-a",
      platformContentId: "note-b",
      type: "normal",
    })).toMatchObject({ ok: false, reason: "identity_mismatch" });
    expect(validateXhsRecordPayload("note", {
      noteId: "note-a",
      platformContentId: "note-a",
      contentType: "video",
    })).toMatchObject({ ok: false, reason: "content_type_missing", path: "payload.type" });
    expect(validateXhsRecordPayload("note", {
      noteId: "note-a",
      platformContentId: "note-a",
      type: "article",
    })).toMatchObject({ ok: false, reason: "content_type_invalid" });
  });

  it("accepts a strict media_inventory/v2 candidate", () => {
    expect(validateXhsMediaInventoryV2({
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [VALID_CANDIDATE],
    })).toEqual({ ok: true });
  });

  it("rejects unknown keys, duplicate slots, invalid cover proof, and URL-shaped identity", () => {
    expect(validateXhsMediaInventoryV2({
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [{ ...VALID_CANDIDATE, width: 1080 }],
    })).toMatchObject({ ok: false, reason: "media_candidate_shape_invalid" });

    expect(validateXhsMediaInventoryV2({
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [VALID_CANDIDATE, { ...VALID_CANDIDATE }],
    })).toMatchObject({ ok: false, reason: "media_slot_duplicate" });

    expect(validateXhsMediaInventoryV2({
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [{ ...VALID_CANDIDATE, coverProvenance: "not_cover" }],
    })).toMatchObject({ ok: false, reason: "media_cover_provenance_invalid" });

    expect(validateXhsMediaInventoryV2({
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [{
        ...VALID_CANDIDATE,
        subject: { kind: "note", noteId: VALID_CANDIDATE.observedAddress, platformContentId: "note-1" },
      }],
    })).toMatchObject({ ok: false, reason: "identity_mismatch" });

    expect(validateXhsMediaInventoryV2({
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [{ ...VALID_CANDIDATE, slotId: "arbitrary-stable-looking-id" }],
    })).toMatchObject({ ok: false, reason: "media_slot_mismatch" });

    expect(validateXhsMediaInventoryV2({
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [{ ...VALID_CANDIDATE, slotId: VALID_CANDIDATE.observedAddress }],
    })).toMatchObject({ ok: false, reason: "media_slot_mismatch" });
  });

  it("rejects an internally valid candidate bound to another emitted subject", () => {
    expect(validateXhsMediaInventorySubjects({
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [{
        ...VALID_CANDIDATE,
        subject: { kind: "note", noteId: "note-2", platformContentId: "note-2" },
        slotId: "note:note-2:cover:image:0",
      }],
    }, [{
      recordKind: "note",
      payload: { noteId: "note-1", platformContentId: "note-1", type: "normal" },
    }])).toMatchObject({ ok: false, reason: "media_subject_mismatch" });
  });

  it("rejects author/avatar candidates because no audited avatar artifact source exists", () => {
    expect(validateXhsMediaInventoryV2({
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [{
        ...VALID_CANDIDATE,
        subject: { kind: "author", authorId: "author-1", platformAuthorId: "author-1" },
        slotId: "author:author-1:avatar:0",
        purpose: "avatar",
      }],
    })).toMatchObject({ ok: false, reason: "media_subject_invalid" });
  });

  it("rejects comment media because no audited comment-media producer exists", () => {
    expect(validateXhsMediaInventoryV2({
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [{
        ...VALID_CANDIDATE,
        subject: { kind: "comment", noteId: "note-1", commentId: "comment-1" },
        slotId: "comment:note-1:comment-1:comment_image:image:0",
        purpose: "comment_image",
      }],
    })).toMatchObject({ ok: false, reason: "media_subject_invalid" });
  });
});
