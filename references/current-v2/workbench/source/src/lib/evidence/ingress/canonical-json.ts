/**
 * Canonical JSON utilities per 07-evidence-ingress-release-b-contract.md §2.
 *
 * Rules:
 * 1. Object keys sorted via Object.keys(value).sort() recursively.
 * 2. Arrays preserve original order.
 * 3. No-space JSON.stringify.
 * 4. Output is UTF-8 bytes.
 */

export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };

/**
 * Validate that a value is a valid JSON value (§2.1).
 * Rejects undefined, NaN, Infinity, BigInt, functions, circular refs, and
 * non-plain objects.
 */
export function assertJsonValue(value: unknown, path = "root"): asserts value is JsonValue {
  if (value === undefined) {
    throw new JsonValidationError(`${path}: undefined is not a valid JSON value`);
  }
  if (typeof value === "number") {
    if (Number.isNaN(value)) throw new JsonValidationError(`${path}: NaN is not a valid JSON value`);
    if (!Number.isFinite(value)) throw new JsonValidationError(`${path}: Infinity is not a valid JSON value`);
    return;
  }
  if (value === null || typeof value === "string" || typeof value === "boolean") {
    return;
  }
  if (typeof value === "bigint") {
    throw new JsonValidationError(`${path}: BigInt is not a valid JSON value`);
  }
  if (typeof value === "function") {
    throw new JsonValidationError(`${path}: function is not a valid JSON value`);
  }
  if (Array.isArray(value)) {
    for (let i = 0; i < value.length; i++) {
      assertJsonValue(value[i], `${path}[${i}]`);
    }
    return;
  }
  if (typeof value === "object") {
    if (!isPlainObject(value)) {
      throw new JsonValidationError(`${path}: non-plain object is not a valid JSON value`);
    }
    for (const key of Object.keys(value)) {
      assertJsonValue((value as Record<string, unknown>)[key], `${path}.${key}`);
    }
    return;
  }
  throw new JsonValidationError(`${path}: unexpected type ${typeof value}`);
}

export class JsonValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "JsonValidationError";
  }
}

/**
 * Check if a value is a plain object (not a class instance, Map, Set, Date, etc.).
 */
function isPlainObject(value: unknown): value is Record<string, unknown> {
  if (value === null || typeof value !== "object") return false;
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

/**
 * Sort object keys recursively, preserving array order (§2.2 rule 1–2).
 * Returns a new value; does not mutate the input.
 */
function sortKeys(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(sortKeys);
  }
  if (value !== null && typeof value === "object" && isPlainObject(value)) {
    const sorted: Record<string, unknown> = {};
    for (const key of Object.keys(value).sort()) {
      sorted[key] = sortKeys((value as Record<string, unknown>)[key]);
    }
    return sorted;
  }
  return value;
}

/**
 * Canonical JSON string: keys sorted, arrays preserved, no spaces (§2.2).
 * Throws if the value contains non-JSON types.
 */
export function canonicalJsonString(value: unknown): string {
  assertJsonValue(value);
  const sorted = sortKeys(value);
  return JSON.stringify(sorted);
}

/**
 * Canonical JSON as UTF-8 bytes (§2.2 rule 4).
 */
export function canonicalJsonBytes(value: unknown): Uint8Array {
  return Buffer.from(canonicalJsonString(value), "utf8");
}
