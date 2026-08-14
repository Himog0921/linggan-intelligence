const { describe, it } = require("node:test");
const assert = require("node:assert/strict");

const {
  XHS_RECORD_PAYLOAD_CONTRACT,
  XHS_MEDIA_INVENTORY_CONTRACT,
  validateXhsRecordPayload,
  validateXhsMediaInventoryV2,
  validateXhsMediaInventorySubjects,
  buildXhsMediaInventoryV2,
} = require("../src/workbench/protocol/v2/xhs-source-contract.cjs");

const NOTE = { noteId: "note-1", platformContentId: "note-1", type: "normal" };
const COVER_ASSET = {
  assetId: "media_note-1_cover-1",
  noteId: "note-1",
  assetType: "image",
  role: "cover",
  ordinal: 0,
  coverProvenance: "platform_explicit",
  sourceUrl: "https://sns-webpic-qc.xhscdn.com/note-1-cover",
};

describe("XHS source contracts", () => {
  it("locks the record and media schema versions", () => {
    assert.equal(XHS_RECORD_PAYLOAD_CONTRACT.schemaVersion, "xhs.record-payload/v2");
    assert.equal(XHS_MEDIA_INVENTORY_CONTRACT.schemaVersion, "xhs.media-inventory/v2");
  });

  it("accepts strict note/author identities and both content types", () => {
    assert.deepEqual(validateXhsRecordPayload("note", NOTE), {
      ok: true,
      identity: "note-1",
      contentType: "normal",
    });
    assert.equal(validateXhsRecordPayload("note", { ...NOTE, type: "video" }).ok, true);
    assert.equal(validateXhsRecordPayload("author", {
      authorId: "author-1",
      platformAuthorId: "author-1",
    }).ok, true);
  });

  it("rejects unequal identities, legacy aliases, and unknown type", () => {
    assert.equal(validateXhsRecordPayload("note", {
      ...NOTE,
      platformContentId: "other-note",
    }).reason, "identity_mismatch");
    assert.equal(validateXhsRecordPayload("note", {
      noteId: "note-1",
      platformContentId: "note-1",
      contentType: "video",
    }).reason, "content_type_missing");
    assert.equal(validateXhsRecordPayload("note", { ...NOTE, type: "article" }).reason, "content_type_invalid");
    assert.equal(validateXhsRecordPayload("author", {
      authorId: "author-1",
      platformAuthorId: "other-author",
    }).reason, "identity_mismatch");
  });

  it("maps real terminal assets to strict stable slots without URL identity", () => {
    const payload = buildXhsMediaInventoryV2({
      notes: [NOTE],
      comments: [],
      authors: [],
      mediaAssets: [COVER_ASSET],
    });
    assert.deepEqual(payload, {
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [{
        subject: { kind: "note", noteId: "note-1", platformContentId: "note-1" },
        slotId: "note:note-1:cover:image:0",
        purpose: "cover",
        kind: "image",
        ordinal: 0,
        observedAddress: COVER_ASSET.sourceUrl,
        coverProvenance: "platform_explicit",
      }],
    });
    assert.ok(!payload.candidates[0].slotId.includes("https"));
  });

  it("rejects cross-subject assets and ambiguous cover provenance", () => {
    assert.throws(
      () => buildXhsMediaInventoryV2({
        notes: [NOTE], comments: [], authors: [],
        mediaAssets: [{ ...COVER_ASSET, noteId: "other-note" }],
      }),
      (error) => error.reason === "media_subject_mismatch",
    );
    assert.throws(
      () => buildXhsMediaInventoryV2({
        notes: [NOTE], comments: [], authors: [],
        mediaAssets: [{ ...COVER_ASSET, coverProvenance: "" }],
      }),
      (error) => error.reason === "media_cover_provenance_invalid",
    );
  });

  it("rejects unknown candidate keys and duplicate stable slots", () => {
    const candidate = buildXhsMediaInventoryV2({
      notes: [NOTE], comments: [], authors: [], mediaAssets: [COVER_ASSET],
    }).candidates[0];
    assert.equal(validateXhsMediaInventoryV2({
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [{ ...candidate, width: 1080 }],
    }).reason, "media_candidate_shape_invalid");
    assert.equal(validateXhsMediaInventoryV2({
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [candidate, { ...candidate }],
    }).reason, "media_slot_duplicate");
    assert.equal(validateXhsMediaInventoryV2({
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [{ ...candidate, slotId: "arbitrary-stable-looking-id" }],
    }).reason, "media_slot_mismatch");
    assert.equal(validateXhsMediaInventoryV2({
      schemaVersion: "xhs.media-inventory/v2",
      candidates: [{ ...candidate, slotId: candidate.observedAddress }],
    }).reason, "media_slot_mismatch");
  });

  it("requires producer ordinals and keeps slots stable when asset arrays reorder", () => {
    assert.throws(
      () => buildXhsMediaInventoryV2({
        notes: [NOTE], comments: [], authors: [],
        mediaAssets: [{ ...COVER_ASSET, ordinal: undefined }],
      }),
      (error) => error.reason === "media_candidate_source_invalid",
    );
    const first = { ...COVER_ASSET, role: "body", coverProvenance: undefined, ordinal: 0, sourceUrl: "https://example.com/0" };
    const second = { ...first, assetId: "opaque-two", ordinal: 1, sourceUrl: "https://example.com/1" };
    const forward = buildXhsMediaInventoryV2({ notes: [NOTE], comments: [], authors: [], mediaAssets: [first, second] });
    const reversed = buildXhsMediaInventoryV2({ notes: [NOTE], comments: [], authors: [], mediaAssets: [second, first] });
    assert.deepEqual(reversed, forward);
  });

  it("rejects author media because no audited avatar artifact source exists", () => {
    assert.throws(
      () => buildXhsMediaInventoryV2({
        notes: [], comments: [],
        authors: [{ authorId: "author-1", platformAuthorId: "author-1" }],
        mediaAssets: [{
          assetId: "author-avatar",
          authorId: "author-1",
          assetType: "image",
          role: "avatar",
          ordinal: 0,
          sourceUrl: "https://example.com/avatar.jpg",
        }],
      }),
      (error) => error.reason === "media_subject_mismatch",
    );
  });

  it("rejects comment media because no audited comment-media producer exists", () => {
    assert.throws(
      () => buildXhsMediaInventoryV2({
        notes: [NOTE],
        comments: [{ noteId: "note-1", commentId: "comment-1" }],
        authors: [],
        mediaAssets: [{
          assetId: "comment-image",
          noteId: "note-1",
          commentId: "comment-1",
          assetType: "image",
          role: "comment_image",
          ordinal: 0,
          sourceUrl: "https://example.com/comment.jpg",
        }],
      }),
      (error) => error.reason === "media_subject_mismatch",
    );
  });

  it("rejects an internally valid candidate bound to another emitted subject", () => {
    const payload = buildXhsMediaInventoryV2({
      notes: [NOTE], comments: [], authors: [], mediaAssets: [COVER_ASSET],
    });
    payload.candidates[0].subject = {
      kind: "note",
      noteId: "note-2",
      platformContentId: "note-2",
    };
    payload.candidates[0].slotId = "note:note-2:cover:image:0";
    assert.equal(validateXhsMediaInventorySubjects(payload, [{
      recordKind: "note",
      payload: NOTE,
    }]).reason, "media_subject_mismatch");
  });
});
