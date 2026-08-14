// @vitest-environment node

import { execFileSync } from "node:child_process";
import { unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { createPrismaClient } from "@/lib/db";
import type { PrismaClient } from "@/lib/prisma-client";

import { XHS_NOTE_DETAIL } from "../contracts/xhs-collection-contracts";
import { CanonicalMediaAdapter } from "../media/canonical-media-adapter";
import { CanonicalMediaSlotWriter } from "../media/canonical-media-slot-writer";
import { ContentProjectionService } from "./content-projection-service";
import {
  addNoteInputToEvaluation,
  installCurrentWithRevocation,
  prepareVerifiedMedia,
  proofCandidate,
  readProofVerifiedMedia,
  seedAcceptedReevaluation,
  seedProofEvaluation,
  seedProjectionObservationWithoutCurrent,
  seedRejectedReevaluation,
  type ProofFixture,
} from "./content-projection-proof-fixtures";

const FIXED_POINT = "38721ece5de80c781a4e059d67beda1388af64a6";
const PREFIX = "content_workbench_b3_projection_proof_";
const PROOF_DB = `${PREFIX}${Date.now()}`;
const PG = {
  host: process.env.V2_PROOF_PG_HOST ?? "127.0.0.1",
  port: process.env.V2_PROOF_PG_PORT ?? "54329",
  user: process.env.V2_PROOF_PG_USER ?? "postgres",
  password: process.env.V2_PROOF_PG_PASSWORD ?? "postgres",
};
const PSQL_URL = `postgresql://${PG.user}:${PG.password}@${PG.host}:${PG.port}/${PROOF_DB}`;
const PRISMA_URL = `${PSQL_URL}?schema=public`;
const PG_ENV = { ...process.env, PGPASSWORD: PG.password };
const OPTED_IN = process.env.B3_PROJECTION_INTEGRATION_DB === "1";
const FIXED_SCHEMA = join(tmpdir(), `b3-projection-fixed-${Date.now()}.prisma`);

let db: PrismaClient;
let db2: PrismaClient;

function psql(database: string, sql: string): string {
  return execFileSync("psql", [
    "-h", PG.host, "-p", PG.port, "-U", PG.user, "-d", database, "-t", "-A", "-c", sql,
  ], { env: PG_ENV, encoding: "utf8" }).trim();
}

function psqlFile(path: string): void {
  execFileSync("psql", [
    "-h", PG.host, "-p", PG.port, "-U", PG.user, "-d", PROOF_DB,
    "-v", "ON_ERROR_STOP=1", "-f", path,
  ], { env: PG_ENV, encoding: "utf8" });
}

function count(table: string, where = "TRUE"): number {
  return Number(psql(PROOF_DB, `SELECT count(*) FROM "${table}" WHERE ${where};`));
}

function scalar(sql: string): string {
  return psql(PROOF_DB, sql);
}

function currentAndUsageBytes(workspaceId: string): string {
  return scalar(`SELECT jsonb_build_object(
    'current', COALESCE((SELECT jsonb_agg(to_jsonb(c) ORDER BY c."contentAssetId") FROM "ContentCurrentProjection" c WHERE c."workspaceId"='${workspaceId}'), '[]'::jsonb),
    'usages', COALESCE((SELECT jsonb_agg(to_jsonb(u) ORDER BY u."id") FROM "ContentMediaUsage" u WHERE u."workspaceId"='${workspaceId}'), '[]'::jsonb)
  )::text;`);
}

function projectionMediaBytes(workspaceId: string): string {
  return scalar(`SELECT jsonb_build_object(
    'current', COALESCE((SELECT jsonb_agg(to_jsonb(c) ORDER BY c."contentAssetId") FROM "ContentCurrentProjection" c WHERE c."workspaceId"='${workspaceId}'), '[]'::jsonb),
    'slots', COALESCE((SELECT jsonb_agg(to_jsonb(s) ORDER BY s."id") FROM "CanonicalMediaSlot" s WHERE s."workspaceId"='${workspaceId}'), '[]'::jsonb),
    'usages', COALESCE((SELECT jsonb_agg(to_jsonb(u) ORDER BY u."id") FROM "ContentMediaUsage" u WHERE u."workspaceId"='${workspaceId}'), '[]'::jsonb)
  )::text;`);
}

function assetCurrentAndUsageBytes(workspaceId: string, platformContentId: string): string {
  return scalar(`SELECT jsonb_build_object(
    'current', COALESCE((SELECT jsonb_agg(to_jsonb(c)) FROM "ContentCurrentProjection" c JOIN "ContentAsset" a ON a."workspaceId"=c."workspaceId" AND a."id"=c."contentAssetId" WHERE c."workspaceId"='${workspaceId}' AND a."platformContentId"='${platformContentId}'), '[]'::jsonb),
    'usages', COALESCE((SELECT jsonb_agg(to_jsonb(u) ORDER BY u."id") FROM "ContentMediaUsage" u JOIN "ContentAsset" a ON a."workspaceId"=u."workspaceId" AND a."id"=u."contentAssetId" WHERE u."workspaceId"='${workspaceId}' AND a."platformContentId"='${platformContentId}'), '[]'::jsonb)
  )::text;`);
}

function service(client: PrismaClient = db): ContentProjectionService {
  return new ContentProjectionService(client, new CanonicalMediaAdapter(client));
}

function explicitBarrier() {
  let releaseWaiter: (() => void) | undefined;
  const released = new Promise<void>((resolve) => { releaseWaiter = resolve; });
  return {
    release: () => releaseWaiter?.(),
    wait: () => released,
  };
}

function installSubjectFenceTestBarrier(): void {
  psql(PROOF_DB, `
    CREATE SEQUENCE "b3_subject_barrier_attempt_seq";
    ALTER FUNCTION "lock_xhs_content_subject"(TEXT,TEXT)
      RENAME TO "lock_xhs_content_subject_candidate";
    CREATE FUNCTION "lock_xhs_content_subject"(p_workspace_id TEXT, p_platform_content_id TEXT)
    RETURNS INTEGER AS $$
    DECLARE app_name TEXT := current_setting('application_name');
    BEGIN
      IF app_name LIKE 'b3-proof-barrier-%' THEN
        PERFORM nextval('"b3_subject_barrier_attempt_seq"');
        PERFORM pg_advisory_xact_lock(hashtextextended('b3-test-' || app_name, 0));
      END IF;
      RETURN "lock_xhs_content_subject_candidate"(p_workspace_id, p_platform_content_id);
    END;
    $$ LANGUAGE plpgsql;
  `);
}

async function withSubjectFenceTestBarrier<T>(run: () => Promise<T>): Promise<T> {
  installSubjectFenceTestBarrier();
  try {
    return await run();
  } finally {
    restoreSubjectFenceAfterTestBarrier();
  }
}

function expectSubjectFenceTestBarrierAbsent(): void {
  expect(scalar(`SELECT (to_regprocedure('public.lock_xhs_content_subject_candidate(text,text)') IS NULL AND to_regclass('public.b3_subject_barrier_attempt_seq') IS NULL)::text;`)).toBe("true");
}

function restoreSubjectFenceAfterTestBarrier(): void {
  psql(PROOF_DB, `
    DROP FUNCTION "lock_xhs_content_subject"(TEXT,TEXT);
    ALTER FUNCTION "lock_xhs_content_subject_candidate"(TEXT,TEXT)
      RENAME TO "lock_xhs_content_subject";
    DROP SEQUENCE "b3_subject_barrier_attempt_seq";
  `);
}

function subjectBarrierAttempts(): number {
  return Number(scalar(`SELECT CASE WHEN is_called THEN last_value ELSE 0 END FROM "b3_subject_barrier_attempt_seq";`));
}

async function waitForAdvisoryBarrier(applicationName: string): Promise<void> {
  for (let attempt = 0; attempt < 500; attempt += 1) {
    const rows = await db.$queryRaw<Array<{ blocked: boolean }>>`
      SELECT EXISTS (
        SELECT 1 FROM pg_stat_activity
        WHERE datname = current_database()
          AND application_name = ${applicationName}
          AND wait_event = 'advisory'
      ) AS blocked
    `;
    if (rows[0]?.blocked) return;
  }
  throw new Error(`Database barrier was not reached by ${applicationName}.`);
}

async function holdDatabaseBarrier(
  client: PrismaClient,
  applicationName: string,
  gate: ReturnType<typeof explicitBarrier>,
  onHeld: () => void,
): Promise<void> {
  await client.$transaction(async (tx) => {
    await tx.$queryRaw<Array<{ locked: number }>>`
      WITH acquired AS (
        SELECT pg_advisory_xact_lock(hashtextextended(${'b3-test-' + applicationName}, 0))
      )
      SELECT 1::INTEGER AS locked FROM acquired
    `;
    onHeld();
    await gate.wait();
  });
}

function installCanonicalSlotInsertTestBarrier(applicationName: string): void {
  psql(PROOF_DB, `
    CREATE FUNCTION "b3_test_pause_canonical_slot_insert"() RETURNS trigger AS $$
    BEGIN
      IF current_setting('application_name') = '${applicationName}' THEN
        PERFORM pg_advisory_xact_lock(hashtextextended('b3-test-${applicationName}', 0));
      END IF;
      RETURN NEW;
    END;
    $$ LANGUAGE plpgsql;
    CREATE TRIGGER "b3_test_pause_canonical_slot_insert"
      BEFORE INSERT ON "CanonicalMediaSlot"
      FOR EACH ROW EXECUTE FUNCTION "b3_test_pause_canonical_slot_insert"();
  `);
}

function restoreCanonicalSlotInsertTestBarrier(): void {
  psql(PROOF_DB, `
    DROP TRIGGER "b3_test_pause_canonical_slot_insert" ON "CanonicalMediaSlot";
    DROP FUNCTION "b3_test_pause_canonical_slot_insert"();
  `);
}

function command(fixture: ProofFixture, verifiedMedia?: Awaited<ReturnType<typeof prepareVerifiedMedia>>) {
  return {
    workspaceId: fixture.workspaceId,
    rawSnapshotId: fixture.rawSnapshotId,
    contractId: XHS_NOTE_DETAIL.id,
    contractVersion: XHS_NOTE_DETAIL.version,
    evaluationId: fixture.evaluationId,
    verifiedMedia,
  };
}

async function expectSqlState(sql: string, state: string): Promise<void> {
  let failure: { status?: number | null; stderr?: string | Buffer } | undefined;
  try {
    execFileSync("psql", [
      "-h", PG.host, "-p", PG.port, "-U", PG.user, "-d", PROOF_DB,
      "-v", "ON_ERROR_STOP=1", "-v", "VERBOSITY=verbose", "-c", sql,
    ], { env: PG_ENV, encoding: "utf8", stdio: "pipe" });
  } catch (error) {
    failure = error as { status?: number | null; stderr?: string | Buffer };
  }
  if (!failure) throw new Error("SQL unexpectedly succeeded");
  expect(failure.status).not.toBe(0);
  expect(failure.stderr?.toString() ?? "").toMatch(new RegExp(`\\b${state}\\b`));
}

beforeAll(async () => {
  if (!OPTED_IN) return;
  execFileSync("psql", [
    "-h", PG.host, "-p", PG.port, "-U", PG.user, "-d", "postgres",
    "-v", "ON_ERROR_STOP=1", "-c", `CREATE DATABASE "${PROOF_DB}"`,
  ], { env: PG_ENV, encoding: "utf8" });
  if (!PROOF_DB.startsWith(PREFIX) || psql(PROOF_DB, "SELECT current_database();") !== PROOF_DB) {
    throw new Error("Fresh proof database identity check failed before schema write.");
  }
  const fixedSchema = execFileSync("git", ["show", `${FIXED_POINT}:prisma/schema.prisma`], { encoding: "utf8" });
  writeFileSync(FIXED_SCHEMA, fixedSchema);
  execFileSync("node", [
    "node_modules/prisma/build/index.js", "db", "push", "--url", PRISMA_URL, "--schema", FIXED_SCHEMA,
  ], { encoding: "utf8", stdio: "pipe", timeout: 180_000 });
  psqlFile("prisma/migrations/20260811120000_add_canonical_media_slot/migration.sql");
  psqlFile("prisma/migrations/20260811150000_add_b3_projection_foundation/migration.sql");
  db = createPrismaClient(PRISMA_URL);
  db2 = createPrismaClient(PRISMA_URL);
  console.log(`[b3-projection-proof] database=${PROOF_DB}`);
}, 240_000);

afterAll(async () => {
  if (OPTED_IN) {
    console.log(`[b3-projection-proof] final-counts CO=${count("ContentObservation")} CCP=${count("ContentCurrentProjection")} CMU=${count("ContentMediaUsage")} MI=${count("MediaItem")} MO=${count("MediaOrigin")} OE=${count("OutboxEvent")}`);
  }
  if (db) await db.$disconnect();
  if (db2) await db2.$disconnect();
  try { unlinkSync(FIXED_SCHEMA); } catch { /* already absent */ }
  if (OPTED_IN) console.log(`[b3-projection-proof] preserved=${PROOF_DB}`);
});

const proof = OPTED_IN ? describe : describe.skip;

proof("B3 Content Projection — 55-case PostgreSQL proof", () => {
  it("01 asserts the exact fresh database before any fixture write", () => {
    expect(scalar("SELECT current_database();")).toBe(PROOF_DB);
  });

  it("02 projects current accepted/full with lifecycle unknown", async () => {
    const fixture = await seedProofEvaluation(db, "02");
    const result = await service().project(command(fixture));
    expect(result.ok).toBe(true);
    expect(scalar(`SELECT "visibilityState"||':'||COALESCE("lifecycleState",'NULL') FROM "ContentCurrentProjection" WHERE "workspaceId"='${fixture.workspaceId}'`)).toBe("visible:NULL");
  });

  it("03 projects current accepted/partial", async () => {
    const fixture = await seedProofEvaluation(db, "03", { completeness: "partial" });
    expect((await service().project(command(fixture))).ok).toBe(true);
  });

  it("04 copies an exact HTTPS XHS sharing URL", async () => {
    const url = "https://www.xiaohongshu.com/explore/proof04?xsec_token=abc";
    const fixture = await seedProofEvaluation(db, "04", { url });
    expect((await service().project(command(fixture))).ok).toBe(true);
    expect(scalar(`SELECT "originalUrl" FROM "ContentCurrentProjection" WHERE "workspaceId"='${fixture.workspaceId}'`)).toBe(url);
  });

  it("05 preserves a missing original URL as null", async () => {
    const fixture = await seedProofEvaluation(db, "05", { url: null });
    expect((await service().project(command(fixture))).ok).toBe(true);
    expect(scalar(`SELECT COALESCE("originalUrl",'NULL') FROM "ContentCurrentProjection" WHERE "workspaceId"='${fixture.workspaceId}'`)).toBe("NULL");
  });

  it("06 rejects non-HTTPS and non-XHS original URLs with zero projection", async () => {
    for (const [label, url] of [["06a", "http://xhslink.com/a"], ["06b", "https://example.com/a"]]) {
      const fixture = await seedProofEvaluation(db, label, { url });
      const before = count("ContentObservation", `"workspaceId"='${fixture.workspaceId}'`);
      const result = await service().project(command(fixture));
      expect(result).toMatchObject({ ok: false, reason: "invalid_original_url" });
      expect(count("ContentObservation", `"workspaceId"='${fixture.workspaceId}'`)).toBe(before);
    }
  });

  it("07 rejects noteId/platformContentId drift", async () => {
    const fixture = await seedProofEvaluation(db, "07", { platformContentId: "different" });
    expect(await service().project(command(fixture))).toMatchObject({ ok: false, reason: "note_identity_mismatch" });
  });

  it("08 rejects a current rejected evaluation", async () => {
    const fixture = await seedProofEvaluation(db, "08", { decision: "rejected" });
    expect(await service().project(command(fixture))).toMatchObject({ ok: false, reason: "evaluation_not_accepted" });
  });

  it("09 converges two independent clients on the same accepted evaluation", async () => {
    const fixture = await seedProofEvaluation(db, "09");
    const [left, right] = await Promise.all([
      service(db).project(command(fixture)), service(db2).project(command(fixture)),
    ]);
    expect(left.ok && right.ok).toBe(true);
    expect(count("ContentObservation", `"workspaceId"='${fixture.workspaceId}'`)).toBe(1);
    expect(count("ContentCurrentProjection", `"workspaceId"='${fixture.workspaceId}'`)).toBe(1);
  });

  it("10 exact replay leaves every business table and version unchanged", async () => {
    const fixture = await seedProofEvaluation(db, "10");
    const first = await service().project(command(fixture));
    const before = ["ContentObservation", "ContentCurrentProjection", "ContentMediaUsage", "MediaItem", "MediaOrigin", "OutboxEvent"].map((table) => count(table));
    const second = await service().project(command(fixture));
    expect(second).toMatchObject({ ok: true, replayed: true });
    expect(["ContentObservation", "ContentCurrentProjection", "ContentMediaUsage", "MediaItem", "MediaOrigin", "OutboxEvent"].map((table) => count(table))).toEqual(before);
    if (first.ok && second.ok) expect(second.version).toBe(first.version);
  });

  it("11 new accepted observation quarantines A then advances B by one version", async () => {
    const a = await seedProofEvaluation(db, "11a", { workspaceId: "ws-11", noteId: "note-11", observedAt: "2026-08-11T08:00:00.000Z" });
    const first = await service().project(command(a));
    const b = await seedProofEvaluation(db, "11b", { workspaceId: "ws-11", noteId: "note-11", observedAt: "2026-08-11T09:00:00.000Z", installCurrent: false });
    await installCurrentWithRevocation(db, b, a.evaluationId);
    expect(scalar(`SELECT "visibilityState" FROM "ContentCurrentProjection" WHERE "workspaceId"='ws-11'`)).toBe("quarantined");
    const second = await service().project(command(b));
    expect(second.ok).toBe(true);
    if (first.ok && second.ok) expect(second.version).toBe(first.version + 1);
  });

  it("12 older observedAt cannot advance a newer current observation", async () => {
    const a = await seedProofEvaluation(db, "12a", { workspaceId: "ws-12", noteId: "note-12", observedAt: "2026-08-11T09:00:00.000Z" });
    await service().project(command(a));
    const b = await seedProofEvaluation(db, "12b", { workspaceId: "ws-12", noteId: "note-12", observedAt: "2026-08-11T08:00:00.000Z", installCurrent: false });
    await installCurrentWithRevocation(db, b, a.evaluationId);
    expect(await service().project(command(b))).toMatchObject({ ok: false, reason: "stale_observation" });
  });

  it("13 equal observedAt with a different observation fails closed", async () => {
    const a = await seedProofEvaluation(db, "13a", { workspaceId: "ws-13", noteId: "note-13" });
    await service().project(command(a));
    const b = await seedProofEvaluation(db, "13b", { workspaceId: "ws-13", noteId: "note-13", installCurrent: false });
    await installCurrentWithRevocation(db, b, a.evaluationId);
    expect(await service().project(command(b))).toMatchObject({ ok: false, reason: "ambiguous_observation_order" });
  });

  it("14 rejects a cross-workspace command before any write", async () => {
    const fixture = await seedProofEvaluation(db, "14");
    expect(await service().project({ ...command(fixture), workspaceId: "other-workspace" })).toMatchObject({ ok: false, reason: "cec_not_current" });
  });

  it("15 composite Canonical/rawRecord provenance rejects a wrong record", async () => {
    const fixture = await seedProofEvaluation(db, "15");
    await service().project(command(fixture));
    const secondAsset = await db.contentAsset.create({ data: {
      workspaceId: fixture.workspaceId, platform: "xhs", platformContentId: "other-15",
      assetKey: `cw-asset:${fixture.workspaceId}:xhs:other-15`, contentCode: "cw-content:global:xhs:other-15",
      contentType: "normal", contentKind: "image_text",
    } });
    await expectSqlState(`INSERT INTO "ContentObservation" ("id","workspaceId","contentAssetId","canonicalObservationId","rawSnapshotId","rawRecordId","observedAt","qualityStatus","adapterVersion") SELECT 'bad-15',co."workspaceId",'${secondAsset.id}',co."canonicalObservationId",co."rawSnapshotId",'wrong-record',co."observedAt",co."qualityStatus",co."adapterVersion" FROM "ContentObservation" co WHERE co."workspaceId"='${fixture.workspaceId}' LIMIT 1;`, "23503");

    const forged = await seedProofEvaluation(db, "15-forged");
    const forgedAsset = await db.contentAsset.create({ data: {
      workspaceId: forged.workspaceId, platform: "xhs", platformContentId: "note-15-forged",
      assetKey: `cw-asset:${forged.workspaceId}:xhs:note-15-forged`,
      contentCode: "cw-content:global:xhs:note-15-forged", contentType: "normal", contentKind: "image_text",
    } });
    const canonicalInsert = (id: string, type: string, title: string, body: string, url: string) =>
      `INSERT INTO "ContentObservation" ("id","workspaceId","contentAssetId","canonicalObservationId","rawSnapshotId","rawRecordId","observedAt","contentType","title","bodyText","originalUrl","fieldPresence","qualityStatus","adapterVersion") SELECT '${id}',co."workspaceId",'${forgedAsset.id}',co."id",co."rawSnapshotId",co."rawRecordId",co."observedAt",${type},${title},${body},${url},co."fieldPresence",co."qualityStatus",nr."adapterVersion" FROM "CanonicalObservation" co JOIN "NormalizationRun" nr ON nr."workspaceId"=co."workspaceId" AND nr."rawSnapshotId"=co."rawSnapshotId" AND nr."id"=co."normalizationRunId" WHERE co."id"='${forged.canonicalObservationId}';`;
    const actualType = `co."payload"#>>'{sourcePayload,type}'`;
    const actualTitle = `co."payload"#>>'{sourcePayload,title}'`;
    const actualBody = `co."payload"#>>'{sourcePayload,content}'`;
    const actualUrl = `co."payload"#>>'{sourcePayload,url}'`;
    await expectSqlState(canonicalInsert("forged-title-15", actualType, "'forged-title'", actualBody, actualUrl), "55000");
    await expectSqlState(canonicalInsert("forged-body-15", actualType, actualTitle, "'forged-body'", actualUrl), "55000");
    await expectSqlState(canonicalInsert("forged-type-15", "'video'", actualTitle, actualBody, actualUrl), "55000");
    await expectSqlState(canonicalInsert("forged-url-15", actualType, actualTitle, actualBody, "'https://www.xiaohongshu.com/explore/other'"), "55000");
  });

  it("16 rejects ContentObservation UPDATE with SQLSTATE 55000", async () => {
    const fixture = await seedProofEvaluation(db, "16");
    await service().project(command(fixture));
    await expectSqlState(`UPDATE "ContentObservation" SET "title"='tampered' WHERE "workspaceId"='${fixture.workspaceId}';`, "55000");
  });

  it("17 rejects ContentObservation DELETE with SQLSTATE 55000", async () => {
    const fixture = await seedProofEvaluation(db, "17");
    await service().project(command(fixture));
    await expectSqlState(`DELETE FROM "ContentObservation" WHERE "workspaceId"='${fixture.workspaceId}';`, "55000");
  });

  it("18 rejects direct CurrentProjection materialized-field mutation", async () => {
    const fixture = await seedProofEvaluation(db, "18");
    await service().project(command(fixture));
    await expectSqlState(`UPDATE "ContentCurrentProjection" SET "title"='tampered' WHERE "workspaceId"='${fixture.workspaceId}';`, "55000");
  });

  it("19 rejects a direct pointer swap even when both URLs are valid XHS links", async () => {
    const a = await seedProofEvaluation(db, "19a", { workspaceId: "ws-19", noteId: "note-19", observedAt: "2026-08-11T08:00:00.000Z" });
    await service().project(command(a));
    const b = await seedProofEvaluation(db, "19b", { workspaceId: "ws-19", noteId: "note-19", observedAt: "2026-08-11T09:00:00.000Z", installCurrent: false });
    await installCurrentWithRevocation(db, b, a.evaluationId);
    await service().project(command(b));
    const oldId = scalar(`SELECT "id" FROM "ContentObservation" WHERE "workspaceId"='ws-19' ORDER BY "observedAt" LIMIT 1`);
    await expectSqlState(`UPDATE "ContentCurrentProjection" SET "currentObservationId"='${oldId}',"originalUrl"='https://www.xiaohongshu.com/explore/note-19' WHERE "workspaceId"='ws-19';`, "55000");
  });

  it("20 deferred database guard rejects CEC advance that omits revocation", async () => {
    const a = await seedProofEvaluation(db, "20a", { workspaceId: "ws-20", noteId: "note-20" });
    await service().project(command(a));
    const b = await seedProofEvaluation(db, "20b", { workspaceId: "ws-20", noteId: "note-20", observedAt: "2026-08-11T09:00:00.000Z", installCurrent: false });
    await expect(db.$transaction((tx) => tx.contractEvaluationCurrent.create({ data: {
      workspaceId: b.workspaceId, rawSnapshotId: b.rawSnapshotId,
      contractId: XHS_NOTE_DETAIL.id, contractVersion: XHS_NOTE_DETAIL.version,
      contractEvaluationId: b.evaluationId, revision: 1,
    } }))).rejects.toThrow(/55000|omitted atomic/);
  });

  it("21 accepted A→B atomically quarantines A and closes only A active V2 usages", async () => {
    const shared = "https://media.invalid/shared-21.jpg";
    const a = await seedProofEvaluation(db, "21a", { workspaceId: "ws-21", noteId: "note-21", candidates: [proofCandidate("note-21", { observedAddress: shared })] });
    const verified = await prepareVerifiedMedia(db, a);
    await service().project(command(a, verified));
    const mediaCount = count("MediaItem", `"workspaceId"='ws-21'`);
    const b = await seedProofEvaluation(db, "21b", { workspaceId: "ws-21", noteId: "note-21", observedAt: "2026-08-11T09:00:00.000Z", installCurrent: false });
    await installCurrentWithRevocation(db, b, a.evaluationId);
    expect(scalar(`SELECT "visibilityState" FROM "ContentCurrentProjection" WHERE "workspaceId"='ws-21'`)).toBe("quarantined");
    expect(count("ContentMediaUsage", `"workspaceId"='ws-21' AND "validTo" IS NULL`)).toBe(0);
    expect(count("MediaItem", `"workspaceId"='ws-21'`)).toBe(mediaCount);
  });

  it("22 B becomes visible only after its complete Projection transaction commits", async () => {
    const a = await seedProofEvaluation(db, "22a", { workspaceId: "ws-22", noteId: "note-22" });
    await service().project(command(a));
    const b = await seedProofEvaluation(db, "22b", { workspaceId: "ws-22", noteId: "note-22", observedAt: "2026-08-11T09:00:00.000Z", installCurrent: false });
    await installCurrentWithRevocation(db, b, a.evaluationId);
    expect(count("ContentCurrentProjection", `"workspaceId"='ws-22' AND "visibilityState"='visible'`)).toBe(0);
    await service().project(command(b));
    expect(count("ContentCurrentProjection", `"workspaceId"='ws-22' AND "visibilityState"='visible' AND "contractEvaluationId"='${b.evaluationId}'`)).toBe(1);
  });

  it("23 re-accepting the same Observation changes evaluation and rebuilds Usage without version increment", async () => {
    const a = await seedProofEvaluation(db, "23", { candidates: [proofCandidate("note-23")] });
    const verified = await prepareVerifiedMedia(db, a);
    const first = await service().project(command(a, verified));
    const reevaluation = await seedAcceptedReevaluation(db, a, "again");
    expect(count("ContentMediaUsage", `"workspaceId"='${a.workspaceId}' AND "validTo" IS NULL`)).toBe(0);
    const second = await service().project(command(reevaluation, verified));
    if (first.ok && second.ok) expect(second.version).toBe(first.version);
    expect(count("ContentMediaUsage", `"workspaceId"='${a.workspaceId}' AND "validTo" IS NULL AND "contractEvaluationId"='${reevaluation.evaluationId}'`)).toBe(1);
  });

  it("24 rejects six-field provenance when only a subset is present", async () => {
    const fixture = await seedProofEvaluation(db, "24");
    await service().project(command(fixture));
    const asset = scalar(`SELECT "contentAssetId" FROM "ContentCurrentProjection" WHERE "workspaceId"='${fixture.workspaceId}'`);
    await db.mediaItem.create({ data: { workspaceId: fixture.workspaceId, kind: "image" } });
    const item = scalar(`SELECT "id" FROM "MediaItem" WHERE "workspaceId"='${fixture.workspaceId}' LIMIT 1`);
    await expectSqlState(`INSERT INTO "ContentMediaUsage" ("id","workspaceId","contentAssetId","mediaItemId","purpose","contractEvaluationId") VALUES ('bad-24','${fixture.workspaceId}','${asset}','${item}','source_cover','${fixture.evaluationId}');`, "55000");
  });

  it("25 rejects a forged Outbox five-field ledger origin", async () => {
    const fixture = await seedProofEvaluation(db, "25", { candidates: [proofCandidate("note-25")] });
    const verified = await prepareVerifiedMedia(db, fixture);
    await service().project(command(fixture, verified));
    const usage = await db.contentMediaUsage.findFirstOrThrow({ where: { workspaceId: fixture.workspaceId } });
    const forged = await db.outboxEvent.create({ data: {
      workspaceId: fixture.workspaceId, eventType: "media.processing_requested",
      aggregateType: "MediaCandidate", aggregateId: "forged-25",
      payload: { workspaceId: fixture.workspaceId, kind: "image", role: "cover", ledgerOrigin: {
        canonicalObservationId: fixture.canonicalObservationId, slotId: usage.canonicalSlotId,
        originId: usage.mediaOriginId, mediaItemId: "wrong", generation: usage.originGeneration,
      } },
    } });
    await expectSqlState(`INSERT INTO "ContentMediaUsage" ("id","workspaceId","contentAssetId","mediaItemId","purpose","contractEvaluationId","canonicalObservationId","canonicalSlotId","mediaOriginId","originGeneration","mediaProcessingEventId") VALUES ('bad-25','${usage.workspaceId}','${usage.contentAssetId}','${usage.mediaItemId}','source_cover','${usage.contractEvaluationId}','${usage.canonicalObservationId}','${usage.canonicalSlotId}','${usage.mediaOriginId}',${usage.originGeneration},'${forged.id}');`, "55000");
  });

  it("26 rejects slot and origin-generation mismatches", async () => {
    const fixture = await seedProofEvaluation(db, "26", { candidates: [proofCandidate("note-26")] });
    const verified = await prepareVerifiedMedia(db, fixture);
    await service().project(command(fixture, verified));
    const usage = await db.contentMediaUsage.findFirstOrThrow({ where: { workspaceId: fixture.workspaceId } });
    await expectSqlState(`INSERT INTO "ContentMediaUsage" ("id","workspaceId","contentAssetId","mediaItemId","purpose","contractEvaluationId","canonicalObservationId","canonicalSlotId","mediaOriginId","originGeneration","mediaProcessingEventId") VALUES ('bad-26a','${usage.workspaceId}','${usage.contentAssetId}','${usage.mediaItemId}','source_image','${usage.contractEvaluationId}','${usage.canonicalObservationId}','wrong-slot','${usage.mediaOriginId}',${usage.originGeneration},'${usage.mediaProcessingEventId}');`, "55000");
    await expectSqlState(`INSERT INTO "ContentMediaUsage" ("id","workspaceId","contentAssetId","mediaItemId","purpose","contractEvaluationId","canonicalObservationId","canonicalSlotId","mediaOriginId","originGeneration","mediaProcessingEventId") VALUES ('bad-26b','${usage.workspaceId}','${usage.contentAssetId}','${usage.mediaItemId}','source_cover','${usage.contractEvaluationId}','${usage.canonicalObservationId}','${usage.canonicalSlotId}','${usage.mediaOriginId}',999,'${usage.mediaProcessingEventId}');`, "55000");
  });

  it("27 makes V2 provenance and business keys immutable", async () => {
    const fixture = await seedProofEvaluation(db, "27", { candidates: [proofCandidate("note-27")] });
    const verified = await prepareVerifiedMedia(db, fixture);
    await service().project(command(fixture, verified));
    await expectSqlState(`UPDATE "ContentMediaUsage" SET "purpose"='source_image' WHERE "workspaceId"='${fixture.workspaceId}';`, "55000");
    await expectSqlState(`UPDATE "OutboxEvent" SET "payload"='{}'::jsonb WHERE "id"=(SELECT "mediaProcessingEventId" FROM "ContentMediaUsage" WHERE "workspaceId"='${fixture.workspaceId}' LIMIT 1);`, "55000");
  });

  it("28 permits validTo null→timestamp exactly once and rejects reopening", async () => {
    const fixture = await seedProofEvaluation(db, "28", { candidates: [proofCandidate("note-28")] });
    const verified = await prepareVerifiedMedia(db, fixture);
    await service().project(command(fixture, verified));
    await seedRejectedReevaluation(db, fixture, "28-rejected");
    expect(count("ContentMediaUsage", `"workspaceId"='${fixture.workspaceId}' AND "validTo" IS NULL`)).toBe(0);
    await expectSqlState(`UPDATE "ContentMediaUsage" SET "validTo"=NULL WHERE "workspaceId"='${fixture.workspaceId}';`, "55000");
  });

  it("29 keeps one physical MediaItem but two proven cover/body usages", async () => {
    const address = "https://media.invalid/shared-29.jpg";
    const candidates = [
      proofCandidate("note-29", { purpose: "cover", observedAddress: address, coverProvenance: "platform_explicit" }),
      proofCandidate("note-29", { purpose: "body", observedAddress: address, ordinal: 0, coverProvenance: "not_cover" }),
    ];
    const fixture = await seedProofEvaluation(db, "29", { candidates });
    const verified = await prepareVerifiedMedia(db, fixture);
    expect((await service().project(command(fixture, verified))).ok).toBe(true);
    expect(count("MediaItem", `"workspaceId"='${fixture.workspaceId}'`)).toBe(1);
    expect(count("ContentMediaUsage", `"workspaceId"='${fixture.workspaceId}' AND "validTo" IS NULL`)).toBe(2);
    expect(scalar(`SELECT string_agg("purpose",',' ORDER BY "purpose") FROM "ContentMediaUsage" WHERE "workspaceId"='${fixture.workspaceId}'`)).toBe("source_cover,source_image");
  });

  it("30 no observed slot means media unknown and synthesizes no state or usage", async () => {
    const fixture = await seedProofEvaluation(db, "30");
    expect((await service().project(command(fixture))).ok).toBe(true);
    expect(count("CanonicalMediaSlot", `"workspaceId"='${fixture.workspaceId}'`)).toBe(0);
    expect(count("ContentMediaUsage", `"workspaceId"='${fixture.workspaceId}'`)).toBe(0);
  });

  it("31 cover ambiguity and live_photo both fail closed before business activation", async () => {
    const ambiguous = await seedProofEvaluation(db, "31a", { candidates: [
      proofCandidate("note-31a", { ordinal: 0 }), proofCandidate("note-31a", { ordinal: 1 }),
    ] });
    const av = await prepareVerifiedMedia(db, ambiguous);
    expect(await service().project(command(ambiguous, av))).toMatchObject({ ok: false, reason: "canonical_cover_ambiguous" });
    const live = await seedProofEvaluation(db, "31b", { candidates: [proofCandidate("note-31b", {
      purpose: "live_photo", kind: "live_photo", coverProvenance: "not_cover",
    })] });
    const lv = await prepareVerifiedMedia(db, live);
    expect(await service().project(command(live, lv))).toMatchObject({ ok: false, reason: "canonical_media_kind_source_incomplete" });
    expect(count("ContentCurrentProjection", `"workspaceId" IN ('${ambiguous.workspaceId}','${live.workspaceId}')`)).toBe(0);
  });

  it("32 a late Usage failure rolls Observation, Current, MediaItem, Origin and Outbox back together", async () => {
    const fixture = await seedProofEvaluation(db, "32", { candidates: [proofCandidate("note-32")] });
    const verified = await prepareVerifiedMedia(db, fixture);
    const before = ["ContentObservation", "ContentCurrentProjection", "ContentMediaUsage", "MediaItem", "MediaOrigin", "OutboxEvent"].map((table) => count(table));
    psql(PROOF_DB, `CREATE OR REPLACE FUNCTION b3_projection_fault() RETURNS trigger AS $$ BEGIN RAISE EXCEPTION 'late fault' USING ERRCODE='55000'; END; $$ LANGUAGE plpgsql; CREATE TRIGGER b3_projection_fault BEFORE INSERT ON "ContentMediaUsage" FOR EACH ROW EXECUTE FUNCTION b3_projection_fault();`);
    await expect(service().project(command(fixture, verified))).rejects.toThrow(/late fault|55000/);
    psql(PROOF_DB, `DROP TRIGGER b3_projection_fault ON "ContentMediaUsage"; DROP FUNCTION b3_projection_fault();`);
    expect(["ContentObservation", "ContentCurrentProjection", "ContentMediaUsage", "MediaItem", "MediaOrigin", "OutboxEvent"].map((table) => count(table))).toEqual(before);
  });

  it("33 proves the SQLSTATE helper rejects a successful SQL command", async () => {
    await expect(expectSqlState("SELECT 1;", "55000"))
      .rejects.toThrow(/unexpectedly succeeded/i);
  });

  it("34 rejects delayed A when a newer cross-snapshot B is already current accepted before any ContentAsset exists", async () => {
    const a = await seedProofEvaluation(db, "34a", { workspaceId: "ws-34", noteId: "note-34", observedAt: "2026-08-11T08:00:00.000Z" });
    const b = await seedProofEvaluation(db2, "34b", { workspaceId: "ws-34", noteId: "note-34", observedAt: "2026-08-11T09:00:00.000Z", installCurrent: false });
    await installCurrentWithRevocation(db2, b, null);
    expect(count("ContentAsset", `"workspaceId"='ws-34'`)).toBe(0);
    expect(await service(db).project(command(a))).toMatchObject({ ok: false, reason: "stale_observation" });
    expect(count("ContentCurrentProjection", `"workspaceId"='ws-34' AND "visibilityState"='visible'`)).toBe(0);
    expect((await service(db2).project(command(b))).ok).toBe(true);
    expect(scalar(`SELECT "contractEvaluationId" FROM "ContentCurrentProjection" WHERE "workspaceId"='ws-34' AND "visibilityState"='visible'`)).toBe(b.evaluationId);
  });

  it("35 keeps newer Current and two active usages byte-identical when an older cross-snapshot accepted arrives", async () => {
    const candidates = [
      proofCandidate("note-35", { purpose: "cover", ordinal: 0, coverProvenance: "platform_explicit" }),
      proofCandidate("note-35", { purpose: "body", ordinal: 0, coverProvenance: "not_cover" }),
    ];
    const newer = await seedProofEvaluation(db, "35-new", { workspaceId: "ws-35", noteId: "note-35", observedAt: "2026-08-11T09:00:00.000Z", candidates });
    await service(db).project(command(newer, await prepareVerifiedMedia(db, newer)));
    const before = currentAndUsageBytes("ws-35");
    const older = await seedProofEvaluation(db2, "35-old", { workspaceId: "ws-35", noteId: "note-35", observedAt: "2026-08-11T08:00:00.000Z", installCurrent: false });
    await installCurrentWithRevocation(db2, older, null);
    expect(currentAndUsageBytes("ws-35")).toBe(before);
    expect(await service(db2).project(command(older))).toMatchObject({ ok: false, reason: "stale_observation" });
    expect(currentAndUsageBytes("ws-35")).toBe(before);
  });

  it("36 keeps newer Current and active usages byte-identical for equal-time different Observation", async () => {
    const newer = await seedProofEvaluation(db, "36-new", { workspaceId: "ws-36", noteId: "note-36", observedAt: "2026-08-11T09:00:00.000Z", candidates: [proofCandidate("note-36")] });
    await service(db).project(command(newer, await prepareVerifiedMedia(db, newer)));
    const before = currentAndUsageBytes("ws-36");
    const ambiguous = await seedProofEvaluation(db2, "36-equal", { workspaceId: "ws-36", noteId: "note-36", observedAt: "2026-08-11T09:00:00.000Z", installCurrent: false });
    await installCurrentWithRevocation(db2, ambiguous, null);
    expect(currentAndUsageBytes("ws-36")).toBe(before);
    expect(await service(db2).project(command(ambiguous))).toMatchObject({ ok: false, reason: "ambiguous_observation_order" });
    expect(currentAndUsageBytes("ws-36")).toBe(before);
  });

  it("37 rejects GUC-spoofed quarantine at COMMIT and preserves Current plus two active usages", async () => {
    const candidates = [
      proofCandidate("note-37", { purpose: "cover", ordinal: 0, coverProvenance: "platform_explicit" }),
      proofCandidate("note-37", { purpose: "body", ordinal: 0, coverProvenance: "not_cover" }),
    ];
    const fixture = await seedProofEvaluation(db, "37", { candidates });
    await service().project(command(fixture, await prepareVerifiedMedia(db, fixture)));
    const before = currentAndUsageBytes(fixture.workspaceId);
    await expectSqlState(`BEGIN; SELECT set_config('content_workbench.b3_projection_mode','revoke',true); UPDATE "ContentCurrentProjection" SET "visibilityState"='quarantined' WHERE "workspaceId"='${fixture.workspaceId}'; COMMIT;`, "55000");
    expect(currentAndUsageBytes(fixture.workspaceId)).toBe(before);
  });

  it("38 rejects direct revoke-function use without a legal CEC supersede and rolls its Usage changes back", async () => {
    const fixture = await seedProofEvaluation(db, "38", { candidates: [proofCandidate("note-38")] });
    await service().project(command(fixture, await prepareVerifiedMedia(db, fixture)));
    const later = await seedProofEvaluation(db, "38-later", { workspaceId: fixture.workspaceId, noteId: "note-38", observedAt: "2026-08-11T09:00:00.000Z", installCurrent: false });
    const before = currentAndUsageBytes(fixture.workspaceId);
    await expectSqlState(`SELECT "revoke_content_projection_for_evaluation"('${fixture.workspaceId}','${fixture.evaluationId}','${later.evaluationId}');`, "55000");
    expect(currentAndUsageBytes(fixture.workspaceId)).toBe(before);
  });

  it("39 rejects direct Current DELETE with SQLSTATE 55000 and preserves two active usages", async () => {
    const candidates = [
      proofCandidate("note-39", { purpose: "cover", ordinal: 0, coverProvenance: "platform_explicit" }),
      proofCandidate("note-39", { purpose: "body", ordinal: 0, coverProvenance: "not_cover" }),
    ];
    const fixture = await seedProofEvaluation(db, "39", { candidates });
    await service().project(command(fixture, await prepareVerifiedMedia(db, fixture)));
    const before = currentAndUsageBytes(fixture.workspaceId);
    await expectSqlState(`DELETE FROM "ContentCurrentProjection" WHERE "workspaceId"='${fixture.workspaceId}';`, "55000");
    expect(currentAndUsageBytes(fixture.workspaceId)).toBe(before);
  });

  it("40 preserves accepted-to-rejected atomic quarantine and closes active usages", async () => {
    const fixture = await seedProofEvaluation(db, "40", { candidates: [proofCandidate("note-40")] });
    await service().project(command(fixture, await prepareVerifiedMedia(db, fixture)));
    await seedRejectedReevaluation(db2, fixture, "rejected");
    expect(count("ContentCurrentProjection", `"workspaceId"='${fixture.workspaceId}' AND "visibilityState"='quarantined'`)).toBe(1);
    expect(count("ContentMediaUsage", `"workspaceId"='${fixture.workspaceId}' AND "validTo" IS NULL`)).toBe(0);
  });

  it("41 retries a stale A snapshot after B commits a newer cross-snapshot CEC behind an explicit barrier", async () => {
    await withSubjectFenceTestBarrier(async () => {
      const applicationName = "b3-proof-barrier-41";
      const delayedDb = createPrismaClient(`${PRISMA_URL}&application_name=${applicationName}`);
      const holderDb = createPrismaClient(PRISMA_URL);
      const gate = explicitBarrier();
      let holder: Promise<void> | undefined;
      try {
        const a = await seedProofEvaluation(db, "41-a", { workspaceId: "ws-41", noteId: "note-41", observedAt: "2026-08-11T08:00:00.000Z" });
        const b = await seedProofEvaluation(db2, "41-b", { workspaceId: "ws-41", noteId: "note-41", observedAt: "2026-08-11T09:00:00.000Z", installCurrent: false });
        let markHeld: (() => void) | undefined;
        const held = new Promise<void>((resolve) => { markHeld = resolve; });
        holder = holdDatabaseBarrier(holderDb, applicationName, gate, () => markHeld?.());
        await held;
        const beforeAttempts = subjectBarrierAttempts();
        const delayedA = service(delayedDb).project(command(a));
        await waitForAdvisoryBarrier(applicationName);
        await installCurrentWithRevocation(db2, b, null);
        gate.release();
        await holder;
        expect(await delayedA).toMatchObject({ ok: false, reason: "stale_observation" });
        expect(subjectBarrierAttempts() - beforeAttempts).toBeGreaterThanOrEqual(2);
        expect(count("ContentCurrentProjection", `"workspaceId"='ws-41' AND "visibilityState"='visible'`)).toBe(0);
        expect((await service(db2).project(command(b))).ok).toBe(true);
        expect(scalar(`SELECT "contractEvaluationId" FROM "ContentCurrentProjection" WHERE "workspaceId"='ws-41' AND "visibilityState"='visible'`)).toBe(b.evaluationId);
      } finally {
        gate.release();
        await holder?.catch(() => undefined);
        await delayedDb.$disconnect();
        await holderDb.$disconnect();
      }
    });
    expectSubjectFenceTestBarrierAbsent();
  });

  it("42 retries the reverse interleaving when newer B snapshots before older A commits its CEC", async () => {
    await withSubjectFenceTestBarrier(async () => {
      const applicationName = "b3-proof-barrier-42";
      const delayedDb = createPrismaClient(`${PRISMA_URL}&application_name=${applicationName}`);
      const holderDb = createPrismaClient(PRISMA_URL);
      const gate = explicitBarrier();
      let holder: Promise<void> | undefined;
      try {
        const b = await seedProofEvaluation(db, "42-b", { workspaceId: "ws-42", noteId: "note-42", observedAt: "2026-08-11T09:00:00.000Z" });
        const a = await seedProofEvaluation(db2, "42-a", { workspaceId: "ws-42", noteId: "note-42", observedAt: "2026-08-11T08:00:00.000Z", installCurrent: false });
        let markHeld: (() => void) | undefined;
        const held = new Promise<void>((resolve) => { markHeld = resolve; });
        holder = holdDatabaseBarrier(holderDb, applicationName, gate, () => markHeld?.());
        await held;
        const beforeAttempts = subjectBarrierAttempts();
        const delayedB = service(delayedDb).project(command(b));
        await waitForAdvisoryBarrier(applicationName);
        await installCurrentWithRevocation(db2, a, null);
        gate.release();
        await holder;
        expect((await delayedB).ok).toBe(true);
        expect(subjectBarrierAttempts() - beforeAttempts).toBeGreaterThanOrEqual(3);
        expect(await service(db2).project(command(a))).toMatchObject({ ok: false, reason: "stale_observation" });
        expect(scalar(`SELECT "contractEvaluationId" FROM "ContentCurrentProjection" WHERE "workspaceId"='ws-42' AND "visibilityState"='visible'`)).toBe(b.evaluationId);
      } finally {
        gate.release();
        await holder?.catch(() => undefined);
        await delayedDb.$disconnect();
        await holderDb.$disconnect();
      }
    });
    expectSubjectFenceTestBarrierAbsent();
  });

  it("43 leaves both subjects byte-identical when one accepted Evaluation contains two note inputs", async () => {
    const candidatesA = [proofCandidate("note-43-a")];
    const candidatesB = [proofCandidate("note-43-b")];
    const currentA = await seedProofEvaluation(db, "43-current-a", { workspaceId: "ws-43", noteId: "note-43-a", observedAt: "2026-08-11T08:00:00.000Z", candidates: candidatesA });
    const currentB = await seedProofEvaluation(db, "43-current-b", { workspaceId: "ws-43", noteId: "note-43-b", observedAt: "2026-08-11T09:00:00.000Z", candidates: candidatesB });
    await service(db).project(command(currentA, await prepareVerifiedMedia(db, currentA)));
    await service(db).project(command(currentB, await prepareVerifiedMedia(db, currentB)));
    const beforeA = assetCurrentAndUsageBytes("ws-43", "note-43-a");
    const beforeB = assetCurrentAndUsageBytes("ws-43", "note-43-b");
    const multi = await seedProofEvaluation(db2, "43-multi-a", { workspaceId: "ws-43", noteId: "note-43-a", observedAt: "2026-08-11T10:00:00.000Z", installCurrent: false });
    await addNoteInputToEvaluation(db2, multi, "43-multi-b", "note-43-b", "2026-08-11T08:00:00.000Z");
    await installCurrentWithRevocation(db2, multi, null);
    expect(assetCurrentAndUsageBytes("ws-43", "note-43-a")).toBe(beforeA);
    expect(assetCurrentAndUsageBytes("ws-43", "note-43-b")).toBe(beforeB);
    expect(await service(db2).project(command(multi))).toMatchObject({ ok: false, reason: "note_input_cardinality_invalid" });
    const assetA = scalar(`SELECT "id" FROM "ContentAsset" WHERE "workspaceId"='ws-43' AND "platformContentId"='note-43-a'`);
    const multiObservation = await seedProjectionObservationWithoutCurrent(db2, multi, assetA);
    await expectSqlState(
      `SELECT * FROM "advance_content_current_projection"('${multi.workspaceId}','${assetA}','${multiObservation.observationId}','${multi.evaluationId}');`,
      "55000",
    );
    expect(assetCurrentAndUsageBytes("ws-43", "note-43-a")).toBe(beforeA);
    expect(assetCurrentAndUsageBytes("ws-43", "note-43-b")).toBe(beforeB);
  });

  it("44 rejects direct controlled advance when observed slots have no active Usage", async () => {
    const fixture = await seedProofEvaluation(db, "44", { candidates: [proofCandidate("note-44")] });
    await prepareVerifiedMedia(db, fixture);
    const seeded = await seedProjectionObservationWithoutCurrent(db, fixture);
    await expectSqlState(`SELECT * FROM "advance_content_current_projection"('${fixture.workspaceId}','${seeded.contentAssetId}','${seeded.observationId}','${fixture.evaluationId}');`, "55000");
    expect(count("ContentCurrentProjection", `"workspaceId"='${fixture.workspaceId}'`)).toBe(0);
  });

  it("45 rejects a GUC-spoofed exact Current insert when observed slots have no active Usage", async () => {
    const fixture = await seedProofEvaluation(db, "45", { candidates: [proofCandidate("note-45")] });
    await prepareVerifiedMedia(db, fixture);
    const seeded = await seedProjectionObservationWithoutCurrent(db, fixture);
    await expectSqlState(`BEGIN; SELECT set_config('content_workbench.b3_projection_mode','advance',true); INSERT INTO "ContentCurrentProjection" ("contentAssetId","workspaceId","currentObservationId","contractEvaluationId","title","bodyText","contentType","publishedAt","authorId","originalUrl","lastObservedAt","projectionVersion","lifecycleState","visibilityState") SELECT co."contentAssetId",co."workspaceId",co."id",'${fixture.evaluationId}',co."title",co."bodyText",co."contentType",co."publishedAt",co."authorId",co."originalUrl",co."observedAt",1,NULL,'visible' FROM "ContentObservation" co WHERE co."id"='${seeded.observationId}'; COMMIT;`, "55000");
    expect(count("ContentCurrentProjection", `"workspaceId"='${fixture.workspaceId}'`)).toBe(0);
  });

  it("46 rejects an exact-field GUC update that would commit a visible Current without Usage", async () => {
    const fixture = await seedProofEvaluation(db, "46");
    await service().project(command(fixture));
    const before = currentAndUsageBytes(fixture.workspaceId);
    await expectSqlState(`BEGIN; INSERT INTO "CanonicalMediaSlot" ("id","workspaceId","canonicalObservationId","slotId","status","kind","ordinal") VALUES ('slot-46','${fixture.workspaceId}','${fixture.canonicalObservationId}','note:${fixture.noteId}:cover:image:0','observed','image',0); SELECT set_config('content_workbench.b3_projection_mode','advance',true); UPDATE "ContentCurrentProjection" SET "updatedAt"=CURRENT_TIMESTAMP WHERE "workspaceId"='${fixture.workspaceId}'; COMMIT;`, "55000");
    expect(currentAndUsageBytes(fixture.workspaceId)).toBe(before);
    expect(count("CanonicalMediaSlot", `"workspaceId"='${fixture.workspaceId}'`)).toBe(0);
  });

  it("47 refuses exact replay success when a pre-existing visible Current has a media completeness gap", async () => {
    const fixture = await seedProofEvaluation(db, "47");
    expect((await service().project(command(fixture))).ok).toBe(true);
    psql(PROOF_DB, `ALTER TABLE "CanonicalMediaSlot" DISABLE TRIGGER USER; INSERT INTO "CanonicalMediaSlot" ("id","workspaceId","canonicalObservationId","slotId","status","kind","ordinal") VALUES ('slot-47','${fixture.workspaceId}','${fixture.canonicalObservationId}','note:${fixture.noteId}:cover:image:0','observed','image',0); ALTER TABLE "CanonicalMediaSlot" ENABLE TRIGGER USER;`);
    try {
      await expect(service().project(command(fixture))).rejects.toThrow(/visible projection media completeness|55000/i);
    } finally {
      psql(PROOF_DB, `ALTER TABLE "CanonicalMediaSlot" DISABLE TRIGGER USER; DELETE FROM "CanonicalMediaSlot" WHERE "id"='slot-47'; ALTER TABLE "CanonicalMediaSlot" ENABLE TRIGGER USER;`);
    }
  });

  it("48 rejects an extra active Usage for a visible Current at COMMIT", async () => {
    const fixture = await seedProofEvaluation(db, "48", { candidates: [proofCandidate("note-48")] });
    await service().project(command(fixture, await prepareVerifiedMedia(db, fixture)));
    const before = currentAndUsageBytes(fixture.workspaceId);
    await expectSqlState(`INSERT INTO "ContentMediaUsage" ("id","workspaceId","contentAssetId","mediaItemId","purpose","ordinal","contractEvaluationId","canonicalObservationId","canonicalSlotId","mediaOriginId","originGeneration","mediaProcessingEventId","observedAt") SELECT 'duplicate-48',"workspaceId","contentAssetId","mediaItemId","purpose","ordinal","contractEvaluationId","canonicalObservationId","canonicalSlotId","mediaOriginId","originGeneration","mediaProcessingEventId","observedAt" FROM "ContentMediaUsage" WHERE "workspaceId"='${fixture.workspaceId}' AND "validTo" IS NULL LIMIT 1;`, "55000");
    expect(currentAndUsageBytes(fixture.workspaceId)).toBe(before);
  });

  it("49 rejects closing or deleting the sole active Usage while Current remains visible", async () => {
    const fixture = await seedProofEvaluation(db, "49", { candidates: [proofCandidate("note-49")] });
    await service().project(command(fixture, await prepareVerifiedMedia(db, fixture)));
    const before = currentAndUsageBytes(fixture.workspaceId);
    await expectSqlState(`UPDATE "ContentMediaUsage" SET "validTo"=CURRENT_TIMESTAMP WHERE "workspaceId"='${fixture.workspaceId}' AND "validTo" IS NULL;`, "55000");
    await expectSqlState(`DELETE FROM "ContentMediaUsage" WHERE "workspaceId"='${fixture.workspaceId}' AND "validTo" IS NULL;`, "55000");
    expect(currentAndUsageBytes(fixture.workspaceId)).toBe(before);
  });

  it("50 restores the subject barrier after an injected assertion failure", async () => {
    await expect(withSubjectFenceTestBarrier(async () => {
      throw new Error("injected-barrier-assertion-failure");
    })).rejects.toThrow("injected-barrier-assertion-failure");
    expectSubjectFenceTestBarrierAbsent();
  });

  it("51 rejects a late direct slot-only insert after a media-unknown Current is visible", async () => {
    const fixture = await seedProofEvaluation(db, "51");
    expect((await service().project(command(fixture))).ok).toBe(true);
    const before = projectionMediaBytes(fixture.workspaceId);
    await expectSqlState(
      `INSERT INTO "CanonicalMediaSlot" ("id","workspaceId","canonicalObservationId","slotId","status","kind","ordinal") VALUES ('slot-51','${fixture.workspaceId}','${fixture.canonicalObservationId}','note:${fixture.noteId}:cover:image:0','observed','image',0);`,
      "55000",
    );
    expect(projectionMediaBytes(fixture.workspaceId)).toBe(before);
  });

  it("52 rejects the real dark slot writer after a media-unknown Current is visible", async () => {
    const fixture = await seedProofEvaluation(db, "52", { candidates: [proofCandidate("note-52")] });
    const verified = await readProofVerifiedMedia(db, fixture);
    expect((await service().project(command(fixture))).ok).toBe(true);
    const before = projectionMediaBytes(fixture.workspaceId);
    await expect(new CanonicalMediaSlotWriter(db2).writeSlots({
      canonicalObservationId: fixture.canonicalObservationId,
      verified,
    })).rejects.toThrow(/media completeness|55000/i);
    expect(projectionMediaBytes(fixture.workspaceId)).toBe(before);
  });

  it("53 serializes a concurrent late slot writer against Current and never commits a visible media gap", async () => {
    const fixture = await seedProofEvaluation(db, "53", { candidates: [proofCandidate("note-53")] });
    const verified = await readProofVerifiedMedia(db, fixture);
    const applicationName = "b3-proof-slot-barrier-53";
    const writerDb = createPrismaClient(`${PRISMA_URL}&application_name=${applicationName}`);
    const holderDb = createPrismaClient(PRISMA_URL);
    const gate = explicitBarrier();
    let holder: Promise<void> | undefined;
    installCanonicalSlotInsertTestBarrier(applicationName);
    try {
      let markHeld: (() => void) | undefined;
      const held = new Promise<void>((resolve) => { markHeld = resolve; });
      holder = holdDatabaseBarrier(holderDb, applicationName, gate, () => markHeld?.());
      await held;
      const writer = new CanonicalMediaSlotWriter(writerDb).writeSlots({
        canonicalObservationId: fixture.canonicalObservationId,
        verified,
      });
      await waitForAdvisoryBarrier(applicationName);
      expect((await service(db2).project(command(fixture))).ok).toBe(true);
      gate.release();
      await holder;
      await expect(writer).rejects.toThrow(/TransactionWriteConflict|media completeness|55000/i);
      expect(count("ContentCurrentProjection", `"workspaceId"='${fixture.workspaceId}' AND "visibilityState"='visible'`)).toBe(1);
      expect(count("CanonicalMediaSlot", `"workspaceId"='${fixture.workspaceId}'`)).toBe(0);
      expect(count("ContentMediaUsage", `"workspaceId"='${fixture.workspaceId}' AND "validTo" IS NULL`)).toBe(0);
      expect(scalar(`SELECT "assert_visible_content_media_completeness"('${fixture.workspaceId}',"contentAssetId") FROM "ContentCurrentProjection" WHERE "workspaceId"='${fixture.workspaceId}';`)).toBe("1");
    } finally {
      gate.release();
      await holder?.catch(() => undefined);
      await writerDb.$disconnect();
      await holderDb.$disconnect();
      restoreCanonicalSlotInsertTestBarrier();
    }
  });

  it("54 commits slot, Domain registration, Usage and Current atomically from verified media", async () => {
    const fixture = await seedProofEvaluation(db, "54", { candidates: [proofCandidate("note-54")] });
    const verified = await readProofVerifiedMedia(db, fixture);
    expect(count("CanonicalMediaSlot", `"workspaceId"='${fixture.workspaceId}'`)).toBe(0);
    expect((await service().project(command(fixture, verified))).ok).toBe(true);
    expect(count("CanonicalMediaSlot", `"workspaceId"='${fixture.workspaceId}'`)).toBe(1);
    expect(count("ContentMediaUsage", `"workspaceId"='${fixture.workspaceId}' AND "validTo" IS NULL`)).toBe(1);
    expect(count("ContentCurrentProjection", `"workspaceId"='${fixture.workspaceId}' AND "visibilityState"='visible'`)).toBe(1);
  });

  it("55 keeps the standalone dark slot writer valid when no visible Current exists", async () => {
    const fixture = await seedProofEvaluation(db, "55", { candidates: [proofCandidate("note-55")] });
    const verified = await readProofVerifiedMedia(db, fixture);
    await expect(new CanonicalMediaSlotWriter(db2).writeSlots({
      canonicalObservationId: fixture.canonicalObservationId,
      verified,
    })).resolves.toMatchObject({ inserted: 1, replayed: 0 });
    expect(count("CanonicalMediaSlot", `"workspaceId"='${fixture.workspaceId}'`)).toBe(1);
    expect(count("ContentCurrentProjection", `"workspaceId"='${fixture.workspaceId}'`)).toBe(0);
  });
});
