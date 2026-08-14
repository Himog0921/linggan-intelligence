/**
 * V2 Database Role Connection Factory — R1-02.
 *
 * Each V2 runtime role uses an independent DSN/connection.
 * Missing/wrong DSN → startup error. No generic DATABASE_URL fallback.
 * No SET ROLE — each client connects with its own login identity.
 */
import { readFileSync, statSync } from "node:fs";

import { createIndependentPrismaClient } from "@/lib/db";
import { PrismaClient } from "@/lib/prisma-client";

export type V2DatabaseRole =
  | "evidence_writer"
  | "adapter_reader"
  | "contract_reader"
  | "canonical_writer"
  | "default_app";

const ROLE_DSN_KEYS: Record<V2DatabaseRole, string> = {
  evidence_writer:  "V2_EVIDENCE_WRITER_DSN",
  adapter_reader:   "V2_ADAPTER_READER_DSN",
  contract_reader:  "V2_CONTRACT_READER_DSN",
  canonical_writer: "V2_CANONICAL_WRITER_DSN",
  default_app:      "V2_DEFAULT_APP_DSN",
};

const ROLE_EXPECTED_USERS: Record<V2DatabaseRole, string> = {
  evidence_writer:  "v2_evidence_writer",
  adapter_reader:   "v2_adapter_reader",
  contract_reader:  "v2_contract_reader",
  canonical_writer: "v2_canonical_writer",
  default_app:      "v2_default_app",
};

const RUNTIME_DSN_FILE_KEY = "V2_RUNTIME_DSN_FILE";

export class V2DatabaseRoleError extends Error {
  constructor(role: V2DatabaseRole, detail: string) {
    super(`V2DatabaseRole[${role}]: ${detail}`);
    this.name = "V2DatabaseRoleError";
  }
}

let roleClients: Partial<Record<V2DatabaseRole, PrismaClient>> = {};

/**
 * Get a Prisma client connected with the specified role's DSN.
 * Caches the client after first creation. Fails if DSN is not configured.
 */
export function getV2DatabaseClient(role: V2DatabaseRole): PrismaClient {
  const existing = roleClients[role];
  if (existing) return existing;

  const key = ROLE_DSN_KEYS[role];
  const dsn = process.env[key] ?? readRuntimeDsnFile()[key];
  if (!dsn) {
    throw new V2DatabaseRoleError(role, `Environment variable ${key} is not set. V2 operations cannot fall back to generic DATABASE_URL.`);
  }

  if (dsn === process.env.DATABASE_URL) {
    throw new V2DatabaseRoleError(role, `DSN must not equal generic DATABASE_URL. Each V2 role requires an independent connection.`);
  }

  const client = createIndependentPrismaClient(dsn);
  roleClients[role] = client;
  return client;
}

let runtimeDsnFileCache: Record<string, string> | null = null;

function readRuntimeDsnFile(): Record<string, string> {
  if (runtimeDsnFileCache) return runtimeDsnFileCache;
  const path = process.env[RUNTIME_DSN_FILE_KEY]?.trim();
  if (!path) return {};
  const stat = statSync(path);
  const runtimeUid = process.getuid?.();
  if (runtimeUid === undefined || !stat.isFile() || (stat.mode & 0o077) !== 0 || stat.uid !== runtimeUid) {
    throw new Error(`${RUNTIME_DSN_FILE_KEY} must be an owner-only regular file owned by the runtime user.`);
  }
  const parsed = JSON.parse(readFileSync(path, "utf8")) as unknown;
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error(`${RUNTIME_DSN_FILE_KEY} must contain a JSON object.`);
  }
  runtimeDsnFileCache = Object.fromEntries(Object.entries(parsed).map(([key, value]) => {
    if (typeof value !== "string" || !value.trim()) {
      throw new Error(`${RUNTIME_DSN_FILE_KEY} has an invalid ${key} value.`);
    }
    return [key, value];
  }));
  return runtimeDsnFileCache;
}

/**
 * Validate that the current session_user matches the expected role identity.
 * Must be called after connecting. Fails on mismatch, SET ROLE, or role membership.
 */
export async function validateV2DatabaseIdentity(client: PrismaClient, role: V2DatabaseRole): Promise<void> {
  const rows = await client.$queryRawUnsafe<Array<{ session_user: string; current_user: string }>>(
    `SELECT session_user, current_user`,
  );
  const row = rows[0];
  if (!row) throw new V2DatabaseRoleError(role, "Could not read session_user.");

  const expected = ROLE_EXPECTED_USERS[role];
  if (row.session_user !== expected) {
    throw new V2DatabaseRoleError(role, `session_user=${row.session_user}, expected=${expected}. Wrong login role or SET ROLE in use.`);
  }
  if (row.current_user !== row.session_user) {
    throw new V2DatabaseRoleError(role, `current_user=${row.current_user} != session_user=${row.session_user}. SET ROLE detected — forbidden.`);
  }
}

/**
 * Disconnect all cached role clients. Use in test teardown.
 */
export async function disconnectV2DatabaseClients(): Promise<void> {
  for (const client of Object.values(roleClients)) {
    await client.$disconnect();
  }
  roleClients = {};
  runtimeDsnFileCache = null;
}

/**
 * For testing only: inject a client for a role.
 */
export function _testInjectV2Client(role: V2DatabaseRole, client: PrismaClient): void {
  roleClients[role] = client;
}
