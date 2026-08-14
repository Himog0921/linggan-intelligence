/**
 * Audited CaptureArtifact verifier for B3 canonical media.
 *
 * Package bytes and registry rows must come from an independently audited
 * Evidence source. There is deliberately no database-backed default source
 * while BLK-015 remains open.
 */

import { createHash } from "node:crypto";

import { validateXhsMediaInventorySubjects, validateXhsMediaInventoryV2 } from "../contracts/xhs-source-contract";
import { type B2SourceSnapshot, verifyB2Source } from "../derived/b2-source-verification";
import type { JsonValue } from "../ingress/canonical-json";

export type EvidenceLifecycleStatus = "ACTIVE" | "ARCHIVED" | "REDACTED" | "PURGED";

export type CaptureArtifactRegistryFact = {
  id: string;
  workspaceId: string;
  rawSnapshotId: string;
  kind: string;
  artifactChecksum: string;
  restricted: boolean;
};

export type EvidenceArtifactAuditReceipt = {
  workspaceId: string;
  rawSnapshotId: string;
  capturePackageId: string;
  accessAuditId: string;
  lifecycleStatus: EvidenceLifecycleStatus;
  integrityStatus: string;
  packageBytes: Uint8Array;
  packageChecksumAlgorithm: string;
  packageChecksumValue: string;
  packageContentLength: number;
  snapshot: B2SourceSnapshot & { capturePackageId: string | null };
  artifacts: CaptureArtifactRegistryFact[];
};

/** Implemented only by the future BLK-015 controlled Evidence reader. */
export interface EvidenceArtifactAuditSource {
  readCaptureArtifactsAndAudit(input: {
    workspaceId: string;
    rawSnapshotId: string;
  }): Promise<EvidenceArtifactAuditReceipt>;
}

export type VerifiedMediaCandidate = Readonly<{
  artifactId: string;
  slotId: string;
  subjectKind: "note";
  noteId: string;
  platformContentId: string;
  purpose: string;
  kind: string;
  ordinal: number;
  observedAddress: string;
  coverProvenance: string;
}>;

type VerifiedMediaArtifactState = Readonly<{
  workspaceId: string;
  rawSnapshotId: string;
  capturePackageId: string;
  accessAuditId: string;
  candidates: readonly VerifiedMediaCandidate[];
}>;

const VERIFIED_MEDIA_ARTIFACT_TOKEN = Symbol("verified-media-artifact");
const verifiedMediaArtifactState = new WeakMap<VerifiedMediaArtifacts, VerifiedMediaArtifactState>();

/** Opaque runtime capability. A same-shaped object is not trusted. */
export class VerifiedMediaArtifacts {
  private readonly _verifiedMediaArtifactBrand = true;

  constructor(token: typeof VERIFIED_MEDIA_ARTIFACT_TOKEN) {
    if (token !== VERIFIED_MEDIA_ARTIFACT_TOKEN) {
      throw new MediaArtifactInvariantError("Forged media Artifact capability.");
    }
  }
}

export type VerifiedArtifactResult =
  | { ok: true; verified: VerifiedMediaArtifacts }
  | { ok: false; reason: string; detail: string };

export class MediaArtifactInvariantError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "MediaArtifactInvariantError";
  }
}

export class VerifiedArtifactReader {
  constructor(private readonly source: EvidenceArtifactAuditSource) {}

  async readAndVerify(input: {
    workspaceId: string;
    rawSnapshotId: string;
  }): Promise<VerifiedArtifactResult> {
    const command = Object.freeze({
      workspaceId: input.workspaceId,
      rawSnapshotId: input.rawSnapshotId,
    });
    const receipt = snapshotReceipt(
      await this.source.readCaptureArtifactsAndAudit(command),
    );

    if (!isEligible(command, receipt)) {
      return fail("evidence_ineligible", "Evidence is not an active or archived verified observation.");
    }

    let capturePackage: ReturnType<typeof verifyB2Source>["capturePackage"];
    try {
      capturePackage = verifyB2Source(receipt.snapshot, {
        packageBytes: receipt.packageBytes,
        packageChecksumAlgorithm: receipt.packageChecksumAlgorithm,
        packageChecksumValue: receipt.packageChecksumValue,
        packageContentLength: receipt.packageContentLength,
      }).capturePackage;
    } catch (error) {
      return fail("capture_package_invalid", error instanceof Error ? error.message : String(error));
    }

    const descriptorCheck = bindArtifactDescriptors(
      receipt.artifacts,
      capturePackage.artifacts,
    );
    if (!descriptorCheck.ok) return descriptorCheck;

    const noteRecords = capturePackage.records.map((record) => ({
      recordKind: record.recordKind,
      payload: record.payload,
    }));
    const candidates: VerifiedMediaCandidate[] = [];

    for (const binding of descriptorCheck.bindings) {
      if (binding.descriptor.kind !== "media_inventory") continue;
      const artifactBytes = Buffer.from(binding.packaged.artifactPayload, "base64");
      const checksum = createHash("sha256").update(artifactBytes).digest("hex");
      if (checksum !== binding.descriptor.artifactChecksum ||
          checksum !== binding.packaged.artifactChecksum ||
          artifactBytes.length !== binding.packaged.contentLength) {
        return fail(
          "artifact_checksum_mismatch",
          `Artifact ${binding.descriptor.id} checksum or content length changed.`,
        );
      }

      let payload: JsonValue;
      try {
        payload = JSON.parse(artifactBytes.toString("utf8")) as JsonValue;
      } catch {
        return fail("artifact_payload_parse_error", `Artifact ${binding.descriptor.id} is not JSON.`);
      }
      const shape = validateXhsMediaInventoryV2(payload);
      if (!shape.ok) return fail(shape.reason, `${binding.descriptor.id}:${shape.path}`);
      const subjects = validateXhsMediaInventorySubjects(payload, noteRecords);
      if (!subjects.ok) return fail(subjects.reason, `${binding.descriptor.id}:${subjects.path}`);

      const inventory = payload as { candidates: Array<Record<string, JsonValue>> };
      for (const candidate of inventory.candidates) {
        const subject = candidate.subject as Record<string, JsonValue>;
        candidates.push(Object.freeze({
          artifactId: binding.descriptor.id,
          slotId: candidate.slotId as string,
          subjectKind: "note",
          noteId: subject.noteId as string,
          platformContentId: subject.platformContentId as string,
          purpose: candidate.purpose as string,
          kind: candidate.kind as string,
          ordinal: candidate.ordinal as number,
          observedAddress: candidate.observedAddress as string,
          coverProvenance: candidate.coverProvenance as string,
        }));
      }
    }

    candidates.sort((left, right) => left.slotId.localeCompare(right.slotId));
    const capability = new VerifiedMediaArtifacts(VERIFIED_MEDIA_ARTIFACT_TOKEN);
    verifiedMediaArtifactState.set(capability, Object.freeze({
      workspaceId: receipt.workspaceId,
      rawSnapshotId: receipt.rawSnapshotId,
      capturePackageId: receipt.capturePackageId,
      accessAuditId: receipt.accessAuditId,
      candidates: Object.freeze(candidates),
    }));
    return { ok: true, verified: capability };
  }
}

export function readVerifiedMediaArtifacts(
  capability: VerifiedMediaArtifacts,
): VerifiedMediaArtifactState {
  const state = verifiedMediaArtifactState.get(capability);
  if (!state) throw new MediaArtifactInvariantError("Media Artifact capability is not authentic.");
  return {
    ...state,
    candidates: state.candidates.map((candidate) => ({ ...candidate })),
  };
}

function snapshotReceipt(receipt: EvidenceArtifactAuditReceipt): EvidenceArtifactAuditReceipt {
  if (!(receipt.packageBytes instanceof Uint8Array) ||
      !receipt.workspaceId || !receipt.rawSnapshotId || !receipt.capturePackageId ||
      !receipt.accessAuditId || !Number.isSafeInteger(receipt.packageContentLength) ||
      receipt.packageContentLength < 0 || !Array.isArray(receipt.artifacts)) {
    throw new MediaArtifactInvariantError("Controlled Evidence reader returned an invalid receipt.");
  }
  return {
    ...structuredClone(receipt),
    packageBytes: Uint8Array.from(receipt.packageBytes),
  };
}

function isEligible(
  command: { workspaceId: string; rawSnapshotId: string },
  receipt: EvidenceArtifactAuditReceipt,
): boolean {
  return receipt.workspaceId === command.workspaceId &&
    receipt.rawSnapshotId === command.rawSnapshotId &&
    receipt.snapshot.workspaceId === command.workspaceId &&
    receipt.snapshot.id === command.rawSnapshotId &&
    receipt.snapshot.capturePackageId === receipt.capturePackageId &&
    receipt.snapshot.integrityStatus === "verified" &&
    receipt.integrityStatus === "verified" &&
    (receipt.lifecycleStatus === "ACTIVE" || receipt.lifecycleStatus === "ARCHIVED") &&
    receipt.artifacts.every((artifact) =>
      artifact.workspaceId === command.workspaceId &&
      artifact.rawSnapshotId === command.rawSnapshotId,
    );
}

type PackagedArtifact = ReturnType<typeof verifyB2Source>["capturePackage"]["artifacts"][number];

function bindArtifactDescriptors(
  descriptors: CaptureArtifactRegistryFact[],
  packagedArtifacts: PackagedArtifact[],
):
  | { ok: true; bindings: Array<{ descriptor: CaptureArtifactRegistryFact; packaged: PackagedArtifact }> }
  | Extract<VerifiedArtifactResult, { ok: false }> {
  if (descriptors.length !== packagedArtifacts.length) {
    return fail("artifact_descriptor_mismatch", "CaptureArtifact count no longer matches CapturePackage.");
  }
  const remaining = [...packagedArtifacts];
  const bindings: Array<{ descriptor: CaptureArtifactRegistryFact; packaged: PackagedArtifact }> = [];
  for (const descriptor of descriptors) {
    const index = remaining.findIndex((artifact) =>
      artifact.kind === descriptor.kind &&
      artifact.artifactChecksum === descriptor.artifactChecksum &&
      artifact.restricted === descriptor.restricted,
    );
    if (index < 0) {
      return fail(
        "artifact_descriptor_mismatch",
        `CaptureArtifact ${descriptor.id} is not bound to a package Artifact.`,
      );
    }
    const [packaged] = remaining.splice(index, 1);
    bindings.push({ descriptor, packaged });
  }
  return { ok: true, bindings };
}

function fail(reason: string, detail: string): Extract<VerifiedArtifactResult, { ok: false }> {
  return { ok: false, reason, detail };
}
