import { describe, expect, it } from "vitest";

import { assertRecordType } from "./legacy-guard";

describe("assertRecordType", () => {
  it("preserves a V1 record type", () => {
    expect(assertRecordType("note", "RawRecord v1-record")).toBe("note");
  });

  it.each([null, undefined, 1, {}])(
    "rejects a non-string V1 record type (%s)",
    (value) => {
      expect(() => assertRecordType(value, "RawRecord v2-record")).toThrow(
        /V1 readers cannot consume it/,
      );
    },
  );
});
