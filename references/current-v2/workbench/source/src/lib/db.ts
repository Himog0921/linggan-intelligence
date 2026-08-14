import { PrismaPg } from "@prisma/adapter-pg";
import { Prisma, PrismaClient } from "../../generated/prisma/client";
import { loadProjectDatabaseEnv } from "./database-url";
import { logSlowPrismaQuery } from "./performance-monitor";

const globalForPrisma = globalThis as unknown as {
  prisma: PrismaClient | undefined;
  prismaPerformanceLoggingAttached: boolean | undefined;
};

loadProjectDatabaseEnv();

export function resolvePrismaDatabaseUrl(
  databaseUrl = process.env.DATABASE_URL,
  nodeEnv = process.env.NODE_ENV,
  connectionLimit = process.env.PRISMA_CONNECTION_LIMIT
) {
  if (!databaseUrl || nodeEnv !== "production") return databaseUrl;

  try {
    const url = new URL(databaseUrl);
    if (url.protocol !== "postgres:" && url.protocol !== "postgresql:") {
      return databaseUrl;
    }
    if (url.hostname === "db.prisma.io") {
      url.hostname = "pooled.db.prisma.io";
    }
    const normalizedConnectionLimit =
      typeof connectionLimit === "string" && /^\d+$/.test(connectionLimit.trim())
        ? connectionLimit.trim()
        : "4";
    if (!url.searchParams.has("connection_limit")) {
      url.searchParams.set("connection_limit", normalizedConnectionLimit);
    }
    if (!url.searchParams.has("pool_timeout")) {
      url.searchParams.set("pool_timeout", "20");
    }
    return url.toString();
  } catch {
    return databaseUrl;
  }
}

const prismaDatabaseUrl = resolvePrismaDatabaseUrl();

export function buildPrismaClientOptions(databaseUrl = prismaDatabaseUrl) {
  if (!databaseUrl) {
    throw new Error("DATABASE_URL is required to initialize Prisma Client.");
  }

  return {
    adapter: new PrismaPg({ connectionString: databaseUrl }),
    log: [{ emit: "event" as const, level: "query" as const }],
  };
}

export function createPrismaClient(databaseUrl = prismaDatabaseUrl) {
  return new PrismaClient(buildPrismaClientOptions(databaseUrl));
}

/**
 * The only approved constructor for an explicit, independently authenticated
 * PostgreSQL identity. Callers must supply a DSN; generic DATABASE_URL
 * fallback remains the responsibility of createPrismaClient only.
 */
export function createIndependentPrismaClient(databaseUrl: string) {
  if (!databaseUrl.trim()) {
    throw new Error("An explicit PostgreSQL DSN is required.");
  }
  return new PrismaClient(buildPrismaClientOptions(databaseUrl));
}

export const prisma = globalForPrisma.prisma ?? createPrismaClient();

type PrismaQueryEventClient = PrismaClient & {
  $on(event: "query", callback: Parameters<typeof logSlowPrismaQuery>[0] extends infer QueryEvent
    ? (event: QueryEvent) => void
    : never): void;
};

if (!globalForPrisma.prismaPerformanceLoggingAttached) {
  (prisma as PrismaQueryEventClient).$on("query", logSlowPrismaQuery);
  globalForPrisma.prismaPerformanceLoggingAttached = true;
}

if (process.env.NODE_ENV !== "production") globalForPrisma.prisma = prisma;

export { Prisma, PrismaClient };
