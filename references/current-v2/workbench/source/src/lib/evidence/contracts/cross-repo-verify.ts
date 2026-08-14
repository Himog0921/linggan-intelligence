/**
 * Cross-repo XHS contract verification (B1-B-03-R1 §4.7).
 *
 * Usage:
 *   npx tsx src/lib/evidence/contracts/cross-repo-verify.ts /path/to/linggan-boom
 *
 * Compares every contract's canonical JSON, contract hash, and fixture package
 * hash between the plugin (source of truth) and the workbench (mirror).
 * Any mismatch is a hard error — exit 0 only when all 6 contracts match.
 */

import { canonicalJsonString, type JsonValue } from "../ingress/canonical-json";
import { CollectionContractRegistry, computeContractHash } from "./collection-contract-registry";
import { XHS_COLLECTION_CONTRACTS, XHS_FIXTURE_PACKAGE_HASHES } from "./xhs-collection-contracts";
import { validateCaptureSubmission } from "../ingress/validator";
import { sha256Hex } from "../ingress/base64";
import { canonicalJsonBytes } from "../ingress/canonical-json";
import {
  evaluateXhsContract,
  normalizeXhsRecord,
  XHS_DERIVED_FIXTURE_HASHES,
  type XhsEvaluationMember,
} from "../derived/xhs-derived-contract";
import { execFileSync } from "node:child_process";
import { join } from "node:path";
import {
  XHS_MEDIA_INVENTORY_CONTRACT,
  XHS_RECORD_PAYLOAD_CONTRACT,
  validateXhsMediaInventoryV2,
  validateXhsMediaInventorySubjects,
  validateXhsRecordPayload,
} from "./xhs-source-contract";

const PLUGIN_REPO = process.argv[2];
if (!PLUGIN_REPO) {
  console.error("Usage: npx tsx cross-repo-verify.ts <path-to-linggan-boom>");
  process.exit(2);
}

const PLUGIN_CONTRACTS_PATH = join(PLUGIN_REPO, "src/workbench/protocol/v2/xhs-contracts.cjs");
const PLUGIN_SOURCE_CONTRACT_PATH = join(PLUGIN_REPO, "src/workbench/protocol/v2/xhs-source-contract.cjs");

// Verify the plugin contracts file exists before spawning any processes.
import { existsSync } from "node:fs";
if (!existsSync(PLUGIN_CONTRACTS_PATH)) {
  console.error(`Plugin contracts file not found: ${PLUGIN_CONTRACTS_PATH}`);
  console.error("Check that the plugin repo path is correct and the file exists.");
  process.exit(1);
}
if (!existsSync(PLUGIN_SOURCE_CONTRACT_PATH)) {
  console.error(`Plugin source contract file not found: ${PLUGIN_SOURCE_CONTRACT_PATH}`);
  process.exit(1);
}

interface PluginSourceContracts {
  recordCanonical: string;
  mediaCanonical: string;
}

function pluginSourceContracts(): PluginSourceContracts {
  const output = execFileSync("node", ["-e", `
    const canonical = require(${JSON.stringify(PLUGIN_CONTRACTS_PATH)}).canonicalJson;
    const source = require(${JSON.stringify(PLUGIN_SOURCE_CONTRACT_PATH)});
    console.log(JSON.stringify({
      recordCanonical: canonical(source.XHS_RECORD_PAYLOAD_CONTRACT),
      mediaCanonical: canonical(source.XHS_MEDIA_INVENTORY_CONTRACT),
    }));
  `], { encoding: "utf8", timeout: 10_000 });
  return JSON.parse(output.trim()) as PluginSourceContracts;
}

interface PluginContract {
  id: string;
  version: number;
  contractHash: string;
  canonicalJson: string;
  fixturePackageHash: string;
  fixtureSubmission: {
    body: {
      header: unknown;
      capturePackage: { checksumValue: string };
    };
    authority: {
      receivedAt: string;
      [key: string]: unknown;
    };
  };
}

function pluginContracts(): PluginContract[] {
  const output = execFileSync("node", ["-e", `
    const mod = require(${JSON.stringify(PLUGIN_CONTRACTS_PATH)});
    const ids = Object.keys(mod.CONTRACTS);
    const results = [];
    for (const id of ids) {
      results.push({
        id,
        version: mod.CONTRACTS[id].version,
        contractHash: mod.contractHash(mod.CONTRACTS[id]),
        canonicalJson: mod.canonicalJson(mod.CONTRACTS[id]),
        fixturePackageHash: mod.FIXTURES[id].fixturePackageHash,
        fixtureSubmission: mod.FIXTURES[id].submission,
      });
    }
    console.log(JSON.stringify(results));
  `], { encoding: "utf8", timeout: 10_000 });

  return JSON.parse(output.trim()) as PluginContract[];
}

function main() {
  const pluginSource = pluginSourceContracts();
  const plugin = pluginContracts();
  const pluginById = new Map(plugin.map((c) => [c.id, c]));
  const registry = new CollectionContractRegistry([...XHS_COLLECTION_CONTRACTS]);

  if (plugin.length !== 6) {
    console.error(`Plugin has ${plugin.length} contracts, expected 6`);
    process.exit(1);
  }

  let failures = 0;

  const workbenchRecordCanonical = canonicalJsonString(XHS_RECORD_PAYLOAD_CONTRACT);
  const workbenchMediaCanonical = canonicalJsonString(XHS_MEDIA_INVENTORY_CONTRACT);
  if (workbenchRecordCanonical !== pluginSource.recordCanonical) {
    console.error("SOURCE_CONTRACT: xhs.record-payload/v2 differs across repos");
    failures++;
  }
  if (workbenchMediaCanonical !== pluginSource.mediaCanonical) {
    console.error("SOURCE_CONTRACT: xhs.media-inventory/v2 differs across repos");
    failures++;
  }

  for (const wb of XHS_COLLECTION_CONTRACTS) {
    const pl = pluginById.get(wb.id);
    if (!pl) {
      console.error(`MISSING: workbench contract ${wb.id} not found in plugin`);
      failures++;
      continue;
    }

    // 1. Version match
    if (wb.version !== pl.version) {
      console.error(`VERSION: ${wb.id} workbench=${wb.version} plugin=${pl.version}`);
      failures++;
    }

    // 2. Contract hash match
    const wbHash = computeContractHash(wb);
    if (wbHash !== pl.contractHash) {
      console.error(`HASH: ${wb.id} workbench=${wbHash} plugin=${pl.contractHash}`);
      failures++;
    }

    // 3. Canonical JSON match (compare sorted JSON directly)
    const wbCanonical = canonicalJsonString(wb);
    const plCanonical = pl.canonicalJson;

    if (wbCanonical !== plCanonical) {
      console.error(`CANONICAL: ${wb.id} differs`);
      console.error(`  workbench: ${wbCanonical}`);
      console.error(`  plugin:    ${plCanonical}`);
      failures++;
    }

    // 4. Validate the actual plugin fixture through the workbench validator
    // and registry. This is the acceptance gate; a hash-only self-check is
    // insufficient because it cannot prove the fixture is ingestible.
    const fixtureSubmission = {
      body: pl.fixtureSubmission.body,
      authority: {
        ...pl.fixtureSubmission.authority,
        // JSON transport serializes Date; restore the runtime type required
        // by the EvidenceIngress authority validator.
        receivedAt: new Date(pl.fixtureSubmission.authority.receivedAt),
      },
    };
    const fixtureResult = validateCaptureSubmission(fixtureSubmission);
    if (!fixtureResult.ok) {
      console.error(`FIXTURE_INVALID: ${wb.id} rejected by workbench validator: ${fixtureResult.reason}`);
      failures++;
      continue;
    }
    const fixtureHeader = fixtureResult.header;
    const fixtureLookup = registry.lookup(
      fixtureHeader.contractId,
      fixtureHeader.contractVersion,
      fixtureHeader.contractHash,
    );
    if (fixtureLookup.status !== "resolved") {
      console.error(`FIXTURE_REGISTRY: ${wb.id} fixture did not resolve (${fixtureLookup.status})`);
      failures++;
      continue;
    }
    const fixtureShapeOk = registry.validateShape(fixtureLookup.definition, {
      recordKinds: fixtureResult.records.map((record) => record.recordKind),
      slotIds: fixtureHeader.report.slots.map((slot) => slot.slotId),
      terminalState: fixtureHeader.report.terminal.state,
      recordsEmpty: fixtureResult.records.length === 0,
    });
    if (!fixtureShapeOk) {
      console.error(`FIXTURE_SHAPE: ${wb.id} rejected by workbench contract shape validation`);
      failures++;
      continue;
    }
    for (const record of fixtureResult.records) {
      const sourceResult = validateXhsRecordPayload(record.recordKind, record.payload);
      if (!sourceResult.ok) {
        console.error(`FIXTURE_SOURCE_CONTRACT: ${wb.id} ${sourceResult.path} ${sourceResult.reason}`);
        failures++;
      }
    }
    for (const artifact of fixtureResult.artifacts.filter((candidate) => candidate.kind === "media_inventory")) {
      let mediaPayload: JsonValue;
      try {
        mediaPayload = JSON.parse(Buffer.from(artifact.artifactPayload, "base64").toString("utf8")) as JsonValue;
      } catch {
        console.error(`FIXTURE_MEDIA_JSON: ${wb.id} media_inventory is not JSON`);
        failures++;
        continue;
      }
      const mediaResult = validateXhsMediaInventoryV2(mediaPayload);
      if (!mediaResult.ok) {
        console.error(`FIXTURE_MEDIA_CONTRACT: ${wb.id} ${mediaResult.path} ${mediaResult.reason}`);
        failures++;
        continue;
      }
      const subjectResult = validateXhsMediaInventorySubjects(
        mediaPayload,
        fixtureResult.records.map((record) => ({ recordKind: record.recordKind, payload: record.payload })),
      );
      if (!subjectResult.ok) {
        console.error(`FIXTURE_MEDIA_SUBJECT: ${wb.id} ${subjectResult.path} ${subjectResult.reason}`);
        failures++;
      }
    }
    if (pl.fixturePackageHash !== pl.fixtureSubmission.body.capturePackage.checksumValue) {
      console.error(`FIXTURE_PACKAGE_HASH: ${wb.id} exported fixture hash disagrees with package checksum`);
      failures++;
      continue;
    }
    if (pl.fixturePackageHash !== XHS_FIXTURE_PACKAGE_HASHES[wb.id]) {
      console.error(`FIXTURE_HASH_LOCK: ${wb.id} plugin=${pl.fixturePackageHash} workbench=${XHS_FIXTURE_PACKAGE_HASHES[wb.id]}`);
      failures++;
      continue;
    }

    if (fixtureHeader.contractId !== wb.id) {
      console.error(`FIXTURE_ID: ${wb.id} header contractId=${fixtureHeader.contractId}`);
      failures++;
    }
    if (fixtureHeader.contractVersion !== wb.version) {
      console.error(`FIXTURE_VERSION: ${wb.id} header version=${fixtureHeader.contractVersion}`);
      failures++;
    }
    if (fixtureHeader.contractHash !== wbHash) {
      console.error(`FIXTURE_HASH: ${wb.id} header hash=${fixtureHeader.contractHash} workbench=${wbHash}`);
      failures++;
    }
    const normalizationResults = fixtureResult.records.map((record) => normalizeXhsRecord({
      id: `fixture:${record.idempotencyKey}`,
      workspaceId: fixtureResult.authority.workspaceId,
      rawSnapshotId: `fixture:${wb.id}`,
      recordKind: record.recordKind,
      platform: record.platform,
      sequence: record.sequence,
      payload: record.payload,
      payloadHash: sha256Hex(canonicalJsonBytes(record.payload)),
      observedAt: new Date(record.observedAt),
    }));
    const members: XhsEvaluationMember[] = normalizationResults.map((normalization, index) => ({
      recordId: `fixture:${fixtureResult.records[index].idempotencyKey}`,
      memberKind: "current",
      runId: `fixture-run:${fixtureResult.records[index].idempotencyKey}`,
      attemptNumber: 1,
      status: normalization.status,
      adapterId: normalization.adapterId,
      adapterVersion: normalization.adapterVersion,
      canonicalSchemaVersion: normalization.canonicalSchemaVersion,
      inputHash: normalization.inputPayloadHash,
      outputHash: normalization.outputPayloadHash,
      sequence: fixtureResult.records[index].sequence,
    }));
    const evaluation = evaluateXhsContract({
      snapshot: {
        workspaceId: fixtureResult.authority.workspaceId,
        rawSnapshotId: `fixture:${wb.id}`,
      },
      eligibility: { lifecycleStatus: "ACTIVE", integrityStatus: "verified" },
      contract: fixtureLookup.definition,
      terminal: fixtureHeader.report.terminal,
      slots: fixtureHeader.report.slots,
      members,
    });
    if (normalizationResults.some((result) => result.status !== "normalized") ||
        evaluation.decision !== "accepted") {
      console.error(`FIXTURE_DERIVED: ${wb.id} was not accepted by the B2 adapter/evaluator`);
      failures++;
      continue;
    }
    const actualNormalizationHashes = normalizationResults.map((result) => result.outputPayloadHash);
    const expectedDerived = XHS_DERIVED_FIXTURE_HASHES[wb.id];
    if (!expectedDerived ||
        JSON.stringify(actualNormalizationHashes) !== JSON.stringify(expectedDerived.normalizationOutputHashes) ||
        evaluation.evaluationInputHash !== expectedDerived.evaluationInputHash) {
      console.error(
        `FIXTURE_DERIVED_HASH: ${wb.id} actual=${JSON.stringify({
          normalizationOutputHashes: actualNormalizationHashes,
          evaluationInputHash: evaluation.evaluationInputHash,
        })}`,
      );
      failures++;
      continue;
    }
    console.log(
      `OK: ${wb.id} contract=${wbHash.slice(0,16)}... fixture=${pl.fixturePackageHash.slice(0,16)}... ` +
      `normalization=${actualNormalizationHashes.join(",")} ` +
      `evaluation=${evaluation.evaluationInputHash}`,
    );
  }

  if (failures > 0) {
    console.error(`\n${failures} cross-repo verification failure(s)`);
    process.exit(1);
  }

  console.log(`\nAll 6 contracts plus record/media source contracts verified across repos.`);
}

main();
