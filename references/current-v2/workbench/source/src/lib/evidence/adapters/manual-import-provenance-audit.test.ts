import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

import { MANUAL_IMPORT_PROVENANCE } from "./manual-import-provenance-audit";

const REPO_ROOT = path.resolve(__dirname, "../../../..");
const ROUTE = readFileSync(
  path.join(REPO_ROOT, "src/app/api/execution-tasks/manual-import/route.ts"),
  "utf8",
);
describe("manual_import authority provenance after DEC-B1-022", () => {
  it("has six traceable facts and no unresolved blockers", () => {
    expect(Object.values(MANUAL_IMPORT_PROVENANCE.facts)).toHaveLength(6);
    expect(
      Object.values(MANUAL_IMPORT_PROVENANCE.facts).every(
        (fact) => fact.verdict === "TRACEABLE",
      ),
    ).toBe(true);
    expect(MANUAL_IMPORT_PROVENANCE.blockers).toEqual([]);
    expect(MANUAL_IMPORT_PROVENANCE.canConstructProductionAuthority).toBe(true);
  });

  it("records the user and plugin authorization identity formats", () => {
    expect(MANUAL_IMPORT_PROVENANCE.facts.sourcePrincipal.sources.join("\n")).toContain(
      "user:<userId>",
    );
    expect(MANUAL_IMPORT_PROVENANCE.facts.importerIdentity.sources.join("\n")).toContain(
      "plugin-authorization:<authorizationId>",
    );
  });

  it("requires authorization/session workspace equality and a server clock", () => {
    expect(MANUAL_IMPORT_PROVENANCE.facts.workspaceId.sources.join("\n")).toContain(
      "manual-import-adapter.ts",
    );
    expect(MANUAL_IMPORT_PROVENANCE.facts.receivedAt.sources.join("\n")).toContain(
      "manual-import-adapter.test.ts",
    );
  });

  it("wires V2 EvidenceIngress into the hard-cut manual import route", () => {
    // R1-05: The existing route now includes V2 EvidenceIngress dispatch.
    expect(ROUTE).toContain("submitManualImportEvidence");
    expect(ROUTE).toContain("validateCaptureSubmissionBody");
    expect(ROUTE).toContain("body: submission.body");
  });
});
