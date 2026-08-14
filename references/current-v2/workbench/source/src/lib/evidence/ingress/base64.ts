/**
 * Strict padded base64 decode per 07-evidence-ingress-release-b-contract.md §3.3.
 *
 * Rules:
 * - RFC 4648 standard padded base64 only.
 * - Rejects base64url, whitespace, missing padding, and lenient decoding.
 * - Re-encoding the decoded bytes must produce the exact input string.
 */

import { createHash } from "node:crypto";

/**
 * Decode a strictly padded standard base64 string.
 * Returns the raw bytes or null if the input is not valid strict base64.
 */
export function decodeStrictBase64(input: string): Uint8Array | null {
  // Must be non-empty and contain only standard base64 characters.
  // No whitespace, no URL-safe chars (- and _ are rejected).
  if (input.length === 0) return null;

  // Check charset: A-Z a-z 0-9 + / and = padding only.
  if (!/^[A-Za-z0-9+/]+={0,2}$/.test(input)) return null;

  // Length must be a multiple of 4.
  if (input.length % 4 !== 0) return null;

  // Padding rules: 1 byte leftover → "==" at end; 2 bytes leftover → "=" at end.
  const padCount = input.endsWith("==") ? 2 : input.endsWith("=") ? 1 : 0;
  // When padCount=2, data chars before padding must leave remainder 2 mod 4.
  // When padCount=1, remainder must be 3. padCount=0, remainder 0 (already checked).
  const expectedRemainder = padCount === 2 ? 2 : padCount === 1 ? 3 : 0;
  if ((input.length - padCount) % 4 !== expectedRemainder) return null;

  // Reject padding in the middle.
  if (input.includes("=") && !input.endsWith(padCount === 2 ? "==" : "=")) return null;

  // Decode using Node.js Buffer (standard base64, not URL-safe).
  let buffer: Buffer;
  try {
    buffer = Buffer.from(input, "base64");
  } catch {
    return null;
  }

  // Round-trip check: re-encode must match input exactly (§3.3).
  const reencoded = buffer.toString("base64");
  if (reencoded !== input) return null;

  // Return Buffer (extends Uint8Array) — Prisma Bytes expects Buffer.
  return buffer;
}

/**
 * Compute lowercase 64-char hex sha256 of a byte array.
 */
export function sha256Hex(bytes: Uint8Array): string {
  return createHash("sha256").update(Buffer.from(bytes)).digest("hex");
}

/**
 * Validate that a string is a lowercase 64-character hex sha256.
 */
export function isLowercaseHex64(value: string): boolean {
  return /^[0-9a-f]{64}$/.test(value);
}

/**
 * Parse an ISO 8601 string with mandatory timezone to a Date.
 * Returns null if the value is not a valid timestamp with Z or ±hh:mm.
 *
 * Per §5.3: reject bare dates, bare local times (no timezone), illegal
 * calendar dates, and auto-normalised values like "Feb 30 → Mar 2".
 */
export function parseIsoTimestamp(value: string): Date | null {
  if (typeof value !== "string" || value.length === 0) return null;

  // Must be ISO 8601 date-time with mandatory timezone designator.
  // Accepted: 2026-08-05T12:00:00Z / 2026-08-05T12:00:00+08:00
  //           2026-08-05T12:00:00.123Z / 2026-08-05T00:30:00-05:00
  // Rejected: bare date, no-timezone, illegal calendar, broken offset.
  const m = value.match(
    /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(Z|[+-]\d{2}:\d{2})$/,
  );
  if (!m) return null;

  const year = parseInt(m[1], 10);
  const month = parseInt(m[2], 10);
  const day = parseInt(m[3], 10);

  // Validate calendar date locally — do NOT convert to UTC first.
  // This correctly rejects Feb 30, Apr 31, etc., and accepts dates
  // near UTC midnight with timezone offsets like 2026-08-05T00:30:00+08:00.
  if (month < 1 || month > 12) return null;
  const daysInMonth = new Date(Date.UTC(year, month, 0)).getUTCDate();
  if (day < 1 || day > daysInMonth) return null;

  // Validate time fields
  const hour = parseInt(m[4], 10);
  const minute = parseInt(m[5], 10);
  const second = parseInt(m[6], 10);
  if (hour > 23 || minute > 59 || second > 59) return null;

  // Validate timezone offset is within range
  const tz = m[7];
  if (tz !== "Z") {
    const offsetMatch = tz.match(/^([+-])(\d{2}):(\d{2})$/);
    if (!offsetMatch) return null;
    const oh = parseInt(offsetMatch[2], 10);
    const om = parseInt(offsetMatch[3], 10);
    if (oh > 23 || om > 59) return null;
  }

  // Now parse via Date — the local validation above guarantees correctness.
  const parsed = new Date(value);
  if (isNaN(parsed.getTime())) return null;

  return parsed;
}
