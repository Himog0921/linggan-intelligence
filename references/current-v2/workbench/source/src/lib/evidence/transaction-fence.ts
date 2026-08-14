import type { Prisma } from "@/lib/prisma-client";

/**
 * A caller-owned authority check executed inside the exact domain transaction.
 * Implementations must lock their authority row and fail closed when ownership
 * expired; an in-memory check is never sufficient.
 */
export type EvidenceTransactionFence = (tx: Prisma.TransactionClient) => Promise<void>;
