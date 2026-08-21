import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

const validFixturePaths = [
  "crates/contracts/tests/fixtures/capture-v1/f01-complete-known-set.json",
  "crates/contracts/tests/fixtures/capture-v1/f01-complete-known-set-payload-extension.json",
];
const invalidRecordHashFixture = {
  path: "crates/contracts/tests/fixtures/capture-v1/f01-package-valid-record-hash-invalid.json",
  expectedInvalidRecordOrdinals: [1],
};
const rfc8785GoldenFixturePath =
  "crates/contracts/tests/fixtures/capture-v1/f01-rfc8785-jcs-golden.json";

function canonicalize(value) {
  if (value === null || typeof value === "string" || typeof value === "boolean") {
    return JSON.stringify(value);
  }

  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new Error("checker supports only finite JSON numbers in these fixtures");
    }
    return JSON.stringify(value);
  }

  if (Array.isArray(value)) {
    return `[${value.map(canonicalize).join(",")}]`;
  }

  if (typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalize(value[key])}`)
      .join(",")}}`;
  }

  throw new Error("checker supports only JSON values");
}

function verifyRecordHashes(fixturePath, packageValue) {
  return packageValue.records
    .map((record) => {
      const literalRecordHash = record.recordHash;
      const recordHashInput = structuredClone(record);
      delete recordHashInput.recordHash;
      const computedRecordHash = `sha256:${createHash("sha256")
        .update(canonicalize(recordHashInput), "utf8")
        .digest("hex")}`;

      return { ordinal: record.ordinal, matches: literalRecordHash === computedRecordHash };
    })
    .filter(({ matches }) => !matches)
    .map(({ ordinal }) => ordinal);
}

function verifyPackageHash(fixturePath, packageValue) {
  const hashInput = structuredClone(packageValue);
  const literalPackageHash = hashInput.packageHash;
  delete hashInput.packageHash;

  const canonicalBytes = canonicalize(hashInput);
  const computedPackageHash = `sha256:${createHash("sha256")
    .update(canonicalBytes, "utf8")
    .digest("hex")}`;

  if (literalPackageHash !== computedPackageHash) {
    throw new Error(
      `${fixturePath}: literal packageHash does not match independent Node JCS/SHA-256 verification`,
    );
  }

}

for (const fixturePath of validFixturePaths) {
  const packageValue = JSON.parse(await readFile(fixturePath, "utf8"));
  const invalidRecordOrdinals = verifyRecordHashes(fixturePath, packageValue);
  if (invalidRecordOrdinals.length !== 0) {
    throw new Error(`${fixturePath}: every literal recordHash must verify`);
  }

  verifyPackageHash(fixturePath, packageValue);

  console.log(`${fixturePath}: recordHash values and packageHash verified`);
}

const invalidPackage = JSON.parse(await readFile(invalidRecordHashFixture.path, "utf8"));
const invalidRecordOrdinals = verifyRecordHashes(invalidRecordHashFixture.path, invalidPackage);
if (
  JSON.stringify(invalidRecordOrdinals) !==
  JSON.stringify(invalidRecordHashFixture.expectedInvalidRecordOrdinals)
) {
  throw new Error(
    `${invalidRecordHashFixture.path}: expected only Record ${invalidRecordHashFixture.expectedInvalidRecordOrdinals.join(", ")} to have a stale recordHash`,
  );
}

verifyPackageHash(invalidRecordHashFixture.path, invalidPackage);
console.log(
  `${invalidRecordHashFixture.path}: packageHash verified and stale Record hash verified as the expected negative case`,
);

const rfc8785GoldenFixture = JSON.parse(
  await readFile(rfc8785GoldenFixturePath, "utf8"),
);
const rfc8785CanonicalUtf8 = canonicalize(rfc8785GoldenFixture.input);
if (rfc8785CanonicalUtf8 !== rfc8785GoldenFixture.expectedCanonicalUtf8) {
  throw new Error(
    `${rfc8785GoldenFixturePath}: expected RFC 8785 canonical UTF-8 text does not match independent Node verification`,
  );
}
if (
  Buffer.from(rfc8785CanonicalUtf8, "utf8").toString("hex") !==
  rfc8785GoldenFixture.expectedCanonicalUtf8Hex
) {
  throw new Error(
    `${rfc8785GoldenFixturePath}: expected RFC 8785 canonical UTF-8 bytes do not match independent Node verification`,
  );
}
const rfc8785ExpectedHash = `sha256:${createHash("sha256")
  .update(rfc8785CanonicalUtf8, "utf8")
  .digest("hex")}`;
if (rfc8785ExpectedHash !== rfc8785GoldenFixture.expectedSha256) {
  throw new Error(
    `${rfc8785GoldenFixturePath}: expected RFC 8785 SHA-256 does not match independent Node verification`,
  );
}
const invalidGoldenRecordOrdinals = verifyRecordHashes(
  rfc8785GoldenFixturePath,
  rfc8785GoldenFixture.capturePackage,
);
if (invalidGoldenRecordOrdinals.length !== 0) {
  throw new Error(`${rfc8785GoldenFixturePath}: every golden Package recordHash must verify`);
}
verifyPackageHash(rfc8785GoldenFixturePath, rfc8785GoldenFixture.capturePackage);
console.log(
  `${rfc8785GoldenFixturePath}: RFC 8785 canonical UTF-8 text/bytes/SHA-256 and embedded Package hashes verified`,
);

console.log(
  "JCS checker scope: verifies these synthetic fixtures with JSON object/array/string/boolean/null and finite JavaScript Number values; it does not detect duplicate keys after JSON.parse, distinguish unsafe integer lexical tokens after JSON.parse, validate unpaired surrogates or complete I-JSON bounds, or enforce resource limits.",
);
