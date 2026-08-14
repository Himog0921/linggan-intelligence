/**
 * 6 canonical XHS CollectionContract definitions per 08-xhs-collection-contracts.md §2.
 *
 * B1-B-03-R1: the plugin repo (linggan-boom) is the source of truth.
 * These definitions are strict mirrors.  Every field must match the plugin's
 * canonical definitions exactly — any deviation in canonical JSON or contract
 * hash is a hard cross-repo failure.
 *
 * DEC-B1-021: these 6 contracts are the ONLY XHS contracts in V2 first phase.
 * No metric RawRecord, no media RawRecord (media goes to media_inventory artifact).
 */

import type { CollectionContractDefinitionV2 } from "../ingress/types";
import { canonicalJsonString } from "../ingress/canonical-json";
import { sha256Hex } from "../ingress/base64";
import {
  XHS_MEDIA_INVENTORY_CONTRACT,
  XHS_RECORD_PAYLOAD_CONTRACT,
} from "./xhs-source-contract";

function sourceContractHash(value: unknown): string {
  return sha256Hex(Buffer.from(canonicalJsonString(value), "utf8"));
}

export const XHS_SOURCE_CONTRACT_BINDINGS = {
  recordPayload: {
    schemaVersion: XHS_RECORD_PAYLOAD_CONTRACT.schemaVersion,
    contractHash: sourceContractHash(XHS_RECORD_PAYLOAD_CONTRACT),
  },
  mediaInventory: {
    schemaVersion: XHS_MEDIA_INVENTORY_CONTRACT.schemaVersion,
    contractHash: sourceContractHash(XHS_MEDIA_INVENTORY_CONTRACT),
  },
} as const;

// ────────────────────────────────────────────────────────────────────────────
// §2: Six fixed contracts
// ────────────────────────────────────────────────────────────────────────────

export const XHS_LIST_SCAN: CollectionContractDefinitionV2 = {
  id: "xhs.list-scan",
  version: 2,
  sourceContracts: XHS_SOURCE_CONTRACT_BINDINGS,
  platforms: ["xhs"],
  recordKinds: ["note"],
  slots: [{ slotId: "note_list", requirement: "required" }],
  terminalPolicy: {
    allowedStates: ["completed", "blocked", "cancelled", "error"],
    allowEmptyRecords: true,
  },
  mediaPolicy: "metadata_only",
};

export const XHS_NOTE_DETAIL: CollectionContractDefinitionV2 = {
  id: "xhs.note-detail",
  version: 2,
  sourceContracts: XHS_SOURCE_CONTRACT_BINDINGS,
  platforms: ["xhs"],
  recordKinds: ["note", "comment"],
  slots: [
    { slotId: "note", requirement: "required" },
    { slotId: "comments", requirement: "conditional" },
  ],
  terminalPolicy: {
    allowedStates: ["completed", "blocked", "cancelled", "error"],
    allowEmptyRecords: true,
  },
  mediaPolicy: "metadata_only",
};

export const XHS_NOTE_FULL: CollectionContractDefinitionV2 = {
  id: "xhs.note-full",
  version: 2,
  sourceContracts: XHS_SOURCE_CONTRACT_BINDINGS,
  platforms: ["xhs"],
  recordKinds: ["note", "comment"],
  slots: [
    { slotId: "note", requirement: "required" },
    { slotId: "comments", requirement: "required" },
  ],
  terminalPolicy: {
    allowedStates: ["completed", "blocked", "cancelled", "error"],
    allowEmptyRecords: true,
  },
  mediaPolicy: "metadata_only",
};

export const XHS_COMMENT_PROBE: CollectionContractDefinitionV2 = {
  id: "xhs.comment-probe",
  version: 2,
  sourceContracts: XHS_SOURCE_CONTRACT_BINDINGS,
  platforms: ["xhs"],
  recordKinds: ["comment"],
  slots: [{ slotId: "comments", requirement: "required" }],
  terminalPolicy: {
    allowedStates: ["completed", "blocked", "cancelled", "error"],
    allowEmptyRecords: true,
  },
  mediaPolicy: "not_required",
};

export const XHS_AUTHOR_PROFILE: CollectionContractDefinitionV2 = {
  id: "xhs.author-profile",
  version: 2,
  sourceContracts: XHS_SOURCE_CONTRACT_BINDINGS,
  platforms: ["xhs"],
  recordKinds: ["author", "note"],
  slots: [
    { slotId: "author", requirement: "required" },
    { slotId: "note_list", requirement: "optional" },
  ],
  terminalPolicy: {
    allowedStates: ["completed", "blocked", "cancelled", "error"],
    allowEmptyRecords: true,
  },
  mediaPolicy: "metadata_only",
};

export const XHS_AUTHOR_LINKS: CollectionContractDefinitionV2 = {
  id: "xhs.author-links",
  version: 2,
  sourceContracts: XHS_SOURCE_CONTRACT_BINDINGS,
  platforms: ["xhs"],
  recordKinds: ["note"],
  slots: [{ slotId: "note_links", requirement: "required" }],
  terminalPolicy: {
    allowedStates: ["completed", "blocked", "cancelled", "error"],
    allowEmptyRecords: true,
  },
  mediaPolicy: "not_required",
};

// ────────────────────────────────────────────────────────────────────────────
// Append-only registry.  New contracts added only after source audit.
// ────────────────────────────────────────────────────────────────────────────

export const XHS_COLLECTION_CONTRACTS: CollectionContractDefinitionV2[] = [
  XHS_LIST_SCAN,
  XHS_NOTE_DETAIL,
  XHS_NOTE_FULL,
  XHS_COMMENT_PROBE,
  XHS_AUTHOR_PROFILE,
  XHS_AUTHOR_LINKS,
];

/**
 * Fixture package hash locks mirrored from the plugin's audited six fixtures.
 * The cross-repo verifier compares these locks before accepting plugin data;
 * changing a fixture therefore cannot pass by changing only the plugin.
 */
export const XHS_FIXTURE_PACKAGE_HASHES: Record<string, string> = {
  "xhs.list-scan": "44a6c191bbc4f1dff1e6a880ad5e8061c4160d5dd432719ae9cbd30bce43368a",
  "xhs.note-detail": "d4591bf425e61d295665a7df3e08b7bde072ffcbaeaf86c4bf0817020920c0e4",
  "xhs.note-full": "1f2ad3680bb478ca2540b603e178cd829e4de5bf05f011911aa79ef5c6aa7f9f",
  "xhs.comment-probe": "69cc3ec11b19fbae081fe81b55ef4aa509122316fed328a1a680798cef7448b2",
  "xhs.author-profile": "882ae3e5c1a8912842a761578ea0c1fa806587e5ceda686aaddbc1ff6c263da8",
  "xhs.author-links": "dc3b782dd86191397d75c4493811798d105d3135ff0474d7740c8e851d08334c",
};
