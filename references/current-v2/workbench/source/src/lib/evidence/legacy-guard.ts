/**
 * Shared guards for V1 nullable legacy fields.
 *
 * After Release-B schema prepare makes recordType nullable, V1 read paths must
 * fail closed instead of inventing a default or cross-reading recordKind.
 *
 * This is the canonical assertion for V1 readers that require recordType.
 */

/**
 * Fail-closed: return the recordType if non-null, otherwise throw.
 *
 * V2 Evidence writes null to recordType and uses recordKind instead.
 * V1 readers MUST NOT read recordKind as a fallback.
 */
export function assertRecordType(value: unknown, context?: string): string {
  if (typeof value !== "string") {
    const ctx = context ? ` for ${context}` : "";
    throw new Error(
      `V1 invariant violation: recordType is not a string${ctx}. ` +
      `This is a V2 Evidence row — V1 readers cannot consume it.`
    );
  }
  return value;
}
