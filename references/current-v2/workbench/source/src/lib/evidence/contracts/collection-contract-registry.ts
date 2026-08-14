/**
 * CollectionContract server-side truth per 07-evidence-ingress-release-b-contract.md §4.
 *
 * Append-only code registry. No Contract database table. Client-reported
 * hashes are NOT trusted — the server computes and compares.
 *
 * B1-B-02 dark core: the production registry contains ZERO contracts. Real XHS
 * contracts are registered in a separate numbered work order after fixture
 * validation. Tests inject explicit fixture definitions.
 */

import {
  canonicalJsonString,
} from "../ingress/canonical-json";
import { sha256Hex } from "../ingress/base64";
import type {
  CollectionContractDefinitionV2,
  CaptureTerminalStateV2,
  RecordKindV2,
  EvidencePlatformV2,
} from "../ingress/types";

export type ContractRegistryLookup =
  | { status: "resolved"; definition: CollectionContractDefinitionV2; expectedHash: string }
  | { status: "not_registered" }
  | { status: "hash_mismatch"; expectedHash: string };

type RegisteredContract = {
  definition: CollectionContractDefinitionV2;
  expectedHash: string;
};

/**
 * Compute the canonical contract hash: sha256(UTF-8(canonicalJson(definition))).
 * The definition is sorted/canonicalized before hashing so field order in the
 * source object does not affect the hash.
 */
export function computeContractHash(definition: CollectionContractDefinitionV2): string {
  return sha256Hex(Buffer.from(canonicalJsonString(definition), "utf8"));
}

/**
 * Validate a CollectionContractDefinitionV2 at load time.
 * Rejects: non-positive version, non-XHS platform, duplicate (id,version),
 * duplicate slotId, duplicate recordKind.
 */
export function validateContractDefinition(def: CollectionContractDefinitionV2): void {
  if (!Number.isInteger(def.version) || def.version < 1) {
    throw new ContractRegistryError(`Contract ${def.id} version must be a positive integer, got ${def.version}`);
  }

  for (const p of def.platforms) {
    if (p !== "xhs") {
      throw new ContractRegistryError(`Contract ${def.id}: non-XHS platform "${p}" is not allowed in B1`);
    }
  }

  const slotIds = new Set<string>();
  for (const slot of def.slots) {
    if (slotIds.has(slot.slotId)) {
      throw new ContractRegistryError(`Contract ${def.id}: duplicate slotId "${slot.slotId}"`);
    }
    slotIds.add(slot.slotId);
  }

  const recordKinds = new Set<RecordKindV2>();
  for (const rk of def.recordKinds) {
    if (recordKinds.has(rk)) {
      throw new ContractRegistryError(`Contract ${def.id}: duplicate recordKind "${rk}"`);
    }
    recordKinds.add(rk);
  }
}

export class ContractRegistryError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ContractRegistryError";
  }
}

/**
 * Injectable append-only contract registry.
 *
 * Production instance is created empty. Tests inject fixture definitions via
 * the constructor.
 */
export class CollectionContractRegistry {
  private readonly byKey = new Map<string, RegisteredContract>();

  constructor(definitions: CollectionContractDefinitionV2[] = []) {
    for (const def of definitions) {
      validateContractDefinition(def);
      const key = `${def.id}:${def.version}`;
      if (this.byKey.has(key)) {
        throw new ContractRegistryError(`Duplicate contract registration: ${key}`);
      }
      this.byKey.set(key, {
        definition: def,
        expectedHash: computeContractHash(def),
      });
    }
  }

  /**
   * Resolve a contract by (id, version) and compare the expected hash.
   */
  lookup(
    id: string,
    version: number,
    clientHash: string
  ): ContractRegistryLookup {
    const key = `${id}:${version}`;
    const registered = this.byKey.get(key);
    if (!registered) {
      return { status: "not_registered" };
    }

    const { definition, expectedHash } = registered;
    if (expectedHash !== clientHash) {
      return { status: "hash_mismatch", expectedHash };
    }

    return { status: "resolved", definition, expectedHash };
  }

  /**
   * Validate that the submission's records and terminal conform to the contract
   * definition (§4 shape validation).
   */
  validateShape(
    definition: CollectionContractDefinitionV2,
    submission: {
      recordKinds: RecordKindV2[];
      slotIds: string[];
      terminalState: CaptureTerminalStateV2;
      recordsEmpty: boolean;
    }
  ): boolean {
    // Every record kind must be in definition.recordKinds.
    for (const rk of submission.recordKinds) {
      if (!definition.recordKinds.includes(rk)) return false;
    }

    // Every reported slotId must be in definition.slots.
    const allowedSlots = new Set(definition.slots.map((s) => s.slotId));
    for (const sid of submission.slotIds) {
      if (!allowedSlots.has(sid)) return false;
    }

    // Terminal state must be allowed.
    if (!definition.terminalPolicy.allowedStates.includes(submission.terminalState)) {
      return false;
    }

    // Empty records must comply with allowEmptyRecords.
    if (submission.recordsEmpty && !definition.terminalPolicy.allowEmptyRecords) {
      return false;
    }

    return true;
  }
}

// Re-export platform type for convenience.
export type { EvidencePlatformV2 };
