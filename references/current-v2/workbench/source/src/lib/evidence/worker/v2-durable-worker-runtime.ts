import { hostname } from "node:os";

import {
  B2DerivedService,
  createEvidenceAnalysisReader,
} from "../derived/b2-derived-service";
import { CanonicalMediaAdapter } from "../media/canonical-media-adapter";
import { VerifiedArtifactReader } from "../media/verified-artifact-reader";
import { ContentProjectionService } from "../projection/content-projection-service";
import { ControlledEvidenceReader } from "../security/controlled-evidence-reader";
import {
  getV2DatabaseClient,
  validateV2DatabaseIdentity,
} from "../security/v2-database-roles";
import { V2DurableWorker } from "./v2-durable-worker";

export async function runV2DurableWorkerTick(batchSize = 5) {
  const worker = await createV2DurableWorker();
  return worker.tick(batchSize);
}

export async function runV2DurableWorkerReceipt(input: {
  workspaceId: string;
  receiptId: string;
}) {
  const worker = await createV2DurableWorker();
  return worker.tickReceipt(input);
}

async function createV2DurableWorker() {
  const contractReaderDb = getV2DatabaseClient("contract_reader");
  const canonicalDb = getV2DatabaseClient("canonical_writer");
  await Promise.all([
    validateV2DatabaseIdentity(contractReaderDb, "contract_reader"),
    validateV2DatabaseIdentity(canonicalDb, "canonical_writer"),
  ]);

  const controlledEvidenceReader = new ControlledEvidenceReader(contractReaderDb);
  const evidenceReader = createEvidenceAnalysisReader(controlledEvidenceReader);
  const b2Service = new B2DerivedService(canonicalDb, evidenceReader);
  const projectionService = new ContentProjectionService(
    canonicalDb,
    new CanonicalMediaAdapter(canonicalDb),
  );
  return new V2DurableWorker({
    db: canonicalDb,
    b2Service,
    artifactReader: new VerifiedArtifactReader(controlledEvidenceReader),
    projectionService,
    workerId: `v2:${hostname()}:${process.pid}`,
  });
}
