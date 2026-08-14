// @vitest-environment node

import pg from "pg";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

const enabled = process.env.V2_SECURITY_INTEGRATION_DB === "1";
const describeIntegration = enabled ? describe : describe.skip;

describeIntegration("V2 Evidence database security boundary", () => {
  const admin = client(requiredEnv("V2_SECURITY_ADMIN_DSN"));
  const adapterReader = client(requiredEnv("V2_ADAPTER_READER_DSN"));
  const contractReader = client(requiredEnv("V2_CONTRACT_READER_DSN"));
  const evidenceWriter = client(requiredEnv("V2_EVIDENCE_WRITER_DSN"));
  const canonicalWriter = client(requiredEnv("V2_CANONICAL_WRITER_DSN"));
  const defaultApp = client(requiredEnv("V2_DEFAULT_APP_DSN"));
  const preflightReader = client(requiredEnv("V2_PREFLIGHT_READONLY_DSN"));
  let database = "";

  beforeAll(async () => {
    await Promise.all([
      admin.connect(),
      adapterReader.connect(),
      contractReader.connect(),
      evidenceWriter.connect(),
      canonicalWriter.connect(),
      defaultApp.connect(),
      preflightReader.connect(),
    ]);
    database = (await admin.query<{ name: string }>("SELECT current_database() name")).rows[0]?.name ?? "";
    if (!database.startsWith("content_workbench_v2_rc_r2_") &&
        !database.startsWith("content_workbench_v2_rc_e2e_")) {
      throw new Error(`Refusing non-proof database ${database || "unknown"}`);
    }
  });

  afterAll(async () => {
    await Promise.all([
      admin.end(),
      adapterReader.end(),
      contractReader.end(),
      evidenceWriter.end(),
      canonicalWriter.end(),
      defaultApp.end(),
      preflightReader.end(),
    ]);
  });

  it("pins every runtime login and the NOLOGIN function owner", async () => {
    await expectIdentity(adapterReader, "v2_adapter_reader");
    await expectIdentity(contractReader, "v2_contract_reader");
    await expectIdentity(evidenceWriter, "v2_evidence_writer");
    await expectIdentity(canonicalWriter, "v2_canonical_writer");
    await expectIdentity(defaultApp, "v2_default_app");
    await expectIdentity(preflightReader, "v2_preflight_reader");
    const owner = await admin.query<{
      owner: string;
      can_login: boolean;
      public_execute: boolean;
      public_schema_usage: boolean;
    }>(`
      SELECT owner.rolname owner,
             owner.rolcanlogin can_login,
             has_function_privilege('public', p.oid, 'EXECUTE') public_execute,
             has_schema_privilege(owner.rolname, 'public', 'USAGE') public_schema_usage
      FROM pg_proc p
      JOIN pg_namespace n ON n.oid=p.pronamespace
      JOIN pg_roles owner ON owner.oid=p.proowner
      WHERE n.nspname='evidence_private' AND p.proname='read_capture_package_and_audit'
    `);
    expect(owner.rows).toEqual([{
      owner: "v2_evidence_reader_owner",
      can_login: false,
      public_execute: false,
      public_schema_usage: true,
    }]);
  });

  it("limits nonce INSERT and expiry cleanup to the adapter identity", async () => {
    const adapterPrivileges = await adapterReader.query<{
      can_insert: boolean;
      can_delete: boolean;
      can_select: boolean;
      can_update: boolean;
    }>(`
      SELECT
        has_table_privilege(current_user, 'public."ExecutionStationRequestNonce"', 'INSERT') can_insert,
        has_table_privilege(current_user, 'public."ExecutionStationRequestNonce"', 'DELETE') can_delete,
        has_table_privilege(current_user, 'public."ExecutionStationRequestNonce"', 'SELECT') can_select,
        has_table_privilege(current_user, 'public."ExecutionStationRequestNonce"', 'UPDATE') can_update
    `);
    expect(adapterPrivileges.rows).toEqual([{
      can_insert: true,
      can_delete: true,
      can_select: false,
      can_update: false,
    }]);

    for (const unauthorized of [evidenceWriter, defaultApp]) {
      const privileges = await unauthorized.query<{ can_insert: boolean; can_delete: boolean }>(`
        SELECT
          has_table_privilege(current_user, 'public."ExecutionStationRequestNonce"', 'INSERT') can_insert,
          has_table_privilege(current_user, 'public."ExecutionStationRequestNonce"', 'DELETE') can_delete
      `);
      expect(privileges.rows).toEqual([{ can_insert: false, can_delete: false }]);
      await expectSqlState(
        unauthorized,
        `INSERT INTO "ExecutionStationRequestNonce" (id,"stationId",nonce,"issuedAt","expiresAt")
         VALUES ('unauthorized-nonce','missing-station','unauthorized',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)`,
        "42501",
      );
    }
  });

  it("returns real bytes and writes an audit bound to session_user", async () => {
    const before = await auditCount(admin);
    const result = await contractReader.query<{
      workspace_id: string;
      raw_snapshot_id: string;
      access_audit_id: string;
      package_bytes: Buffer;
      capture_package_id: string;
      snapshot_metadata: { id: string; records: unknown[] };
      artifact_descriptors: unknown[];
    }>("SELECT * FROM evidence_private.read_capture_package_and_audit($1,$2,$3,$4)", [
      "ws-r2", "snap-r2", "b2_normalization", "security-integration",
    ]);
    expect(result.rows).toHaveLength(1);
    expect(result.rows[0]).toMatchObject({ workspace_id: "ws-r2", raw_snapshot_id: "snap-r2" });
    expect(result.rows[0]?.package_bytes.length).toBeGreaterThan(0);
    expect(result.rows[0]?.capture_package_id).toBeTruthy();
    expect(result.rows[0]?.snapshot_metadata).toMatchObject({ id: "snap-r2" });
    expect(Array.isArray(result.rows[0]?.snapshot_metadata.records)).toBe(true);
    expect(Array.isArray(result.rows[0]?.artifact_descriptors)).toBe(true);
    const audit = await admin.query<{ accessedBy: string; requestTraceId: string }>(
      `SELECT "accessedBy", "requestTraceId" FROM "EvidenceAccessAudit" WHERE id=$1`,
      [result.rows[0]?.access_audit_id],
    );
    expect(audit.rows).toEqual([{ accessedBy: "v2_contract_reader", requestTraceId: "security-integration" }]);
    expect(await auditCount(admin)).toBe(before + 1);
  });

  it("rejects direct bytes, cross-workspace, restricted evidence and unauthorized callers", async () => {
    for (const runtimeRole of [adapterReader, contractReader, evidenceWriter, canonicalWriter, defaultApp]) {
      await expectSqlState(runtimeRole, `SELECT "packagePayload" FROM "CapturePackage" LIMIT 1`, "42501");
      await expectSqlState(runtimeRole, `SELECT payload FROM "RawRecord" LIMIT 1`, "42501");
      await expectSqlState(runtimeRole, `SELECT "payloadClob" FROM "RawSnapshot" LIMIT 1`, "42501");
      await expectSqlState(runtimeRole, `SELECT "storageKey" FROM "RawSnapshot" LIMIT 1`, "42501");
    }
    await expectSqlState(
      contractReader,
      "SELECT * FROM evidence_private.read_capture_package_and_audit('wrong-workspace','snap-r2','b2_normalization',NULL)",
      "42501",
    );
    await expectSqlState(
      contractReader,
      "SELECT * FROM evidence_private.read_capture_package_and_audit('ws-r2','snap-restricted','b2_normalization',NULL)",
      "42501",
    );
    await expectSqlState(
      evidenceWriter,
      "SELECT * FROM evidence_private.read_capture_package_and_audit('ws-r2','snap-r2','b2_normalization',NULL)",
      "42501",
    );
  });

  it("makes workspace grants and access audits fail closed", async () => {
    await admin.query(`UPDATE "EvidenceReaderWorkspaceGrant" SET "revokedAt"=CURRENT_TIMESTAMP
      WHERE "workspaceId"='ws-r2' AND "readerRole"='v2_contract_reader'`);
    try {
      await expectSqlState(
        contractReader,
        "SELECT * FROM evidence_private.read_capture_package_and_audit('ws-r2','snap-r2','b2_normalization',NULL)",
        "42501",
      );
    } finally {
      await admin.query(`UPDATE "EvidenceReaderWorkspaceGrant" SET "revokedAt"=NULL
        WHERE "workspaceId"='ws-r2' AND "readerRole"='v2_contract_reader'`);
    }
    await expectSqlState(contractReader, `UPDATE "EvidenceAccessAudit" SET "accessReason"='tampered'`, "42501");
    await expectSqlState(contractReader, `DELETE FROM "EvidenceAccessAudit"`, "42501");
  });

  it("forces the production preflight identity to remain read-only", async () => {
    const state = await preflightReader.query<{ read_only: string }>(
      "SELECT current_setting('transaction_read_only') read_only",
    );
    expect(state.rows).toEqual([{ read_only: "on" }]);
    await expectSqlState(preflightReader, `UPDATE "V2DurableWork" SET status='dead'`, "25006");
  });
});

function client(dsn: string): pg.Client {
  return new pg.Client({ connectionString: dsn });
}

function requiredEnv(name: string): string {
  if (!enabled) return "postgresql://disabled";
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is required when V2_SECURITY_INTEGRATION_DB=1`);
  return value;
}

async function expectIdentity(connection: pg.Client, expected: string): Promise<void> {
  const result = await connection.query<{ session_user: string; current_user: string }>(
    "SELECT session_user, current_user",
  );
  expect(result.rows).toEqual([{ session_user: expected, current_user: expected }]);
}

async function expectSqlState(connection: pg.Client, sql: string, expected: string): Promise<void> {
  try {
    await connection.query(sql);
  } catch (error) {
    expect((error as { code?: string }).code).toBe(expected);
    return;
  }
  throw new Error(`Expected SQLSTATE ${expected}, but SQL succeeded: ${sql}`);
}

async function auditCount(connection: pg.Client): Promise<number> {
  const result = await connection.query<{ count: string }>(`SELECT count(*) count FROM "EvidenceAccessAudit"`);
  return Number(result.rows[0]?.count ?? 0);
}
