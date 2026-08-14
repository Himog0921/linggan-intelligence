// @vitest-environment node

import { describe, expect, it } from "vitest";

import {
  normalizeXhsRecord,
  type XhsRawRecordFact,
} from "./xhs-derived-contract";

describe("XHS Derived contract", () => {
  it("normalizes a complete note into the fixed technical canonical form", () => {
    const record: XhsRawRecordFact = {
      id: "record-1",
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      recordKind: "note",
      platform: "xhs",
      sequence: 0,
      payload: {
        noteId: "note 1",
        platformContentId: "note 1",
        type: "normal",
        title: "title",
        content: "body",
        url: "https://xhs.test/note/1",
      },
      payloadHash: null,
      observedAt: new Date("2026-08-05T11:58:00.000Z"),
    };

    expect(normalizeXhsRecord(record)).toEqual({
      adapterId: "xhs.note",
      adapterVersion: "2.0.0",
      canonicalSchemaVersion: "xhs.canonical/2",
      status: "normalized",
      inputPayloadHash: "df8b6fedc183b0f6c74232b3218f7a4ef8cabe0c92b9ef06244ad613c202f8c7",
      outputPayloadHash: "171083c146f81a5af3587b06107c3936a43fd0de9ca5afcbf3831f7a57bded40",
      missingFields: null,
      parseErrors: null,
      observation: {
        observationKind: "note",
        subjectKey: "xhs:note:note%201",
        observedAt: new Date("2026-08-05T11:58:00.000Z"),
        schemaVersion: "xhs.canonical/2",
        payload: {
          platform: "xhs",
          recordKind: "note",
          subjectKey: "xhs:note:note%201",
          observedAt: "2026-08-05T11:58:00.000Z",
          sourcePayload: {
            noteId: "note 1",
            platformContentId: "note 1",
            type: "normal",
            title: "title",
            content: "body",
            url: "https://xhs.test/note/1",
          },
        },
        payloadHash: "171083c146f81a5af3587b06107c3936a43fd0de9ca5afcbf3831f7a57bded40",
        qualityStatus: "complete",
        fieldPresence: {
          content: true,
          noteId: true,
          platformContentId: true,
          title: true,
          type: true,
          url: true,
        },
      },
    });
  });

  it("scopes the same comment id to its note id", () => {
    const base: XhsRawRecordFact = {
      id: "comment-record-1",
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      recordKind: "comment",
      platform: "xhs",
      sequence: 0,
      payload: {
        noteId: "note-a",
        commentId: "shared-comment",
        text: "comment",
      },
      payloadHash: null,
      observedAt: new Date("2026-08-05T11:58:00.000Z"),
    };

    const first = normalizeXhsRecord(base);
    const second = normalizeXhsRecord({
      ...base,
      id: "comment-record-2",
      payload: { ...base.payload as Record<string, string>, noteId: "note-b" },
    });

    expect(first.observation?.subjectKey).toBe("xhs:comment:note-a:shared-comment");
    expect(second.observation?.subjectKey).toBe("xhs:comment:note-b:shared-comment");
    expect(first.observation?.subjectKey).not.toBe(second.observation?.subjectKey);
  });

  it("rejects a comment without its parent note identity", () => {
    const result = normalizeXhsRecord({
      id: "comment-record-missing-note",
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      recordKind: "comment",
      platform: "xhs",
      sequence: 0,
      payload: { commentId: "comment-1", text: "comment" },
      payloadHash: null,
      observedAt: new Date("2026-08-05T11:58:00.000Z"),
    });

    expect(result).toMatchObject({
      adapterId: "xhs.comment",
      status: "rejected",
      outputPayloadHash: null,
      missingFields: ["noteId"],
      parseErrors: [{ path: "payload.noteId", code: "identity_missing" }],
      observation: null,
    });
  });

  it("uses strict equal author identities and records partial fields", () => {
    const result = normalizeXhsRecord({
      id: "author-record-1",
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      recordKind: "author",
      platform: "xhs",
      sequence: 0,
      payload: {
        authorId: "author/1",
        platformAuthorId: "author/1",
        name: "Author",
      },
      payloadHash: null,
      observedAt: new Date("2026-08-05T11:58:00.000Z"),
    });

    expect(result).toMatchObject({
      adapterId: "xhs.author",
      status: "normalized",
      missingFields: ["profileUrl"],
      observation: {
        subjectKey: "xhs:author:author%2F1",
        qualityStatus: "partial",
        fieldPresence: {
          authorId: true,
          platformAuthorId: true,
          name: true,
          profileUrl: false,
        },
      },
    });
  });

  it("rejects unequal note and author identities before ContractEvaluation can accept", () => {
    const note = normalizeXhsRecord({
      id: "note-identity-mismatch",
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      recordKind: "note",
      platform: "xhs",
      sequence: 0,
      payload: { noteId: "note-a", platformContentId: "note-b", type: "normal" },
      payloadHash: null,
      observedAt: new Date("2026-08-05T11:58:00.000Z"),
    });
    const author = normalizeXhsRecord({
      id: "author-identity-mismatch",
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      recordKind: "author",
      platform: "xhs",
      sequence: 0,
      payload: { authorId: "author-a", platformAuthorId: "author-b" },
      payloadHash: null,
      observedAt: new Date("2026-08-05T11:58:00.000Z"),
    });

    expect(note).toMatchObject({
      status: "rejected",
      parseErrors: [{ path: "payload.noteId,platformContentId", code: "identity_mismatch" }],
      observation: null,
    });
    expect(author).toMatchObject({
      status: "rejected",
      parseErrors: [{ path: "payload.authorId,platformAuthorId", code: "identity_mismatch" }],
      observation: null,
    });
  });

  it("rejects missing, aliased, and unknown note content types", () => {
    const payloads: XhsRawRecordFact["payload"][] = [
      { noteId: "note-1", platformContentId: "note-1" },
      { noteId: "note-1", platformContentId: "note-1", contentType: "video" },
      { noteId: "note-1", platformContentId: "note-1", type: "article" },
    ];
    for (const payload of payloads) {
      expect(normalizeXhsRecord({
        id: "note-type-invalid",
        workspaceId: "workspace-1",
        rawSnapshotId: "snapshot-1",
        recordKind: "note",
        platform: "xhs",
        sequence: 0,
        payload,
        payloadHash: null,
        observedAt: new Date("2026-08-05T11:58:00.000Z"),
      })).toMatchObject({ status: "rejected", observation: null });
    }
  });

  it("quarantines a record whose stored payload hash does not match its payload", () => {
    const result = normalizeXhsRecord({
      id: "note-record-hash-mismatch",
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      recordKind: "note",
      platform: "xhs",
      sequence: 0,
      payload: { noteId: "note-1" },
      payloadHash: "f".repeat(64),
      observedAt: new Date("2026-08-05T11:58:00.000Z"),
    });

    expect(result).toMatchObject({
      adapterId: "xhs.note",
      status: "quarantined",
      outputPayloadHash: null,
      parseErrors: [{ path: "payload", code: "hash_mismatch" }],
      observation: null,
    });
  });

  it("turns an unsupported record kind into an auditable rejected result", () => {
    const result = normalizeXhsRecord({
      id: "metric-record-1",
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      recordKind: "metric",
      platform: "xhs",
      sequence: 0,
      payload: { viewCount: 10 },
      payloadHash: null,
      observedAt: new Date("2026-08-05T11:58:00.000Z"),
    });

    expect(result).toMatchObject({
      adapterId: "xhs.unsupported",
      status: "rejected",
      outputPayloadHash: null,
      parseErrors: [{ path: "recordKind", code: "record_kind_unsupported" }],
      observation: null,
    });
  });
});
