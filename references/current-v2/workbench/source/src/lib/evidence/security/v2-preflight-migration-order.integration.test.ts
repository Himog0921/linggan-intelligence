// @vitest-environment node

import pg from "pg";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

const enabled = process.env.V2_PREFLIGHT_ORDER_INTEGRATION_DB === "1";
const describeIntegration = enabled ? describe : describe.skip;

describeIntegration("V2 preflight identity exists before the hard-cut migration", () => {
  const admin = client(requiredEnv("V2_PREFLIGHT_ORDER_ADMIN_DSN"));
  const preflight = client(requiredEnv("V2_PREFLIGHT_ORDER_READER_DSN"));
  let database = "";

  beforeAll(async () => {
    await Promise.all([admin.connect(), preflight.connect()]);
    database = (await admin.query<{ name: string }>(
      "SELECT current_database() name",
    )).rows[0]?.name ?? "";
    if (!database.startsWith("content_workbench_v2_preflight_order_")) {
      throw new Error(`Refusing non-order-proof database ${database || "unknown"}`);
    }
  });

  afterAll(async () => {
    await Promise.all([admin.end(), preflight.end()]);
    if (database) console.log(`[v2-preflight-order] Preserving proof database ${database}.`);
  });

  it("connects as the read-only preflight role after expand while hard cut is absent", async () => {
    const hardCut = await admin.query<{ reader_function: string | null }>(`
      SELECT to_regprocedure(
        'evidence_private.read_capture_package_and_audit(text,text,text,text)'
      )::text reader_function
    `);
    expect(hardCut.rows).toEqual([{ reader_function: null }]);

    const identity = await preflight.query<{
      database: string;
      session_user: string;
      current_user: string;
      read_only: string;
    }>(`
      SELECT current_database() database, session_user, current_user,
             current_setting('transaction_read_only') read_only
    `);
    expect(identity.rows).toEqual([{
      database,
      session_user: "v2_preflight_reader",
      current_user: "v2_preflight_reader",
      read_only: "on",
    }]);

    const grants = await preflight.query<{
      station_select: boolean;
      work_select: boolean;
      receipt_select: boolean;
      migration_select: boolean;
      work_update: boolean;
    }>(`
      SELECT
        has_table_privilege(current_user, 'public."ExecutionStation"', 'SELECT') station_select,
        has_table_privilege(current_user, 'public."V2DurableWork"', 'SELECT') work_select,
        has_table_privilege(current_user, 'public."EvidenceIngressReceipt"', 'SELECT') receipt_select,
        has_table_privilege(current_user, 'public."_prisma_migrations"', 'SELECT') migration_select,
        has_table_privilege(current_user, 'public."V2DurableWork"', 'UPDATE') work_update
    `);
    expect(grants.rows).toEqual([{
      station_select: true,
      work_select: true,
      receipt_select: true,
      migration_select: true,
      work_update: false,
    }]);
    await expect(preflight.query(`SELECT count(*) FROM "ExecutionStation"`)).resolves.toBeDefined();
    await expect(preflight.query(`SELECT count(*) FROM "V2DurableWork"`)).resolves.toBeDefined();
    await expect(preflight.query(
      `SELECT migration_name, checksum, finished_at, rolled_back_at FROM "_prisma_migrations"`,
    )).resolves.toBeDefined();
    await expectSqlState(
      preflight,
      `UPDATE "V2DurableWork" SET status='dead'`,
      "25006",
    );
  });
});

function client(dsn: string): pg.Client {
  return new pg.Client({ connectionString: dsn });
}

function requiredEnv(name: string): string {
  if (!enabled) return "postgresql://disabled";
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is required for the preflight order proof`);
  return value;
}

async function expectSqlState(
  connection: pg.Client,
  sql: string,
  expected: string,
): Promise<void> {
  try {
    await connection.query(sql);
  } catch (error) {
    expect((error as { code?: string }).code).toBe(expected);
    return;
  }
  throw new Error(`Expected SQLSTATE ${expected}, but SQL succeeded: ${sql}`);
}
