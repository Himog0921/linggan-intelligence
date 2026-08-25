import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  FIRST_DISCOVERY_CONTRACT_VERSION,
  buildFirstDiscoveryPackage
} from "../src/discovery-contract.js";

const packageDirectory = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourceDirectory = join(packageDirectory, "src");
const manifest = JSON.parse(readFileSync(join(sourceDirectory, "manifest.json"), "utf8"));

assert.equal(manifest.manifest_version, 3);
assert.deepEqual(manifest.permissions, []);
assert.deepEqual(manifest.host_permissions, ["http://localhost:3000/*"]);
assert.equal("content_scripts" in manifest, false);
assert.equal("externally_connectable" in manifest, false);
assert.equal("web_accessible_resources" in manifest, false);

const releaseSource = readdirSync(sourceDirectory)
  .sort()
  .map((file) => readFileSync(join(sourceDirectory, file), "utf8"))
  .join("\n");

for (const forbiddenRuntimeReference of [
  "lingganboom.fun",
  "linggan boom",
  "content workbench",
  "xhscdn",
  "activeTab",
  "cookies",
  "downloads",
  "scripting",
  "content_scripts",
  "ocr",
  "asr"
]) {
  assert.equal(
    releaseSource.toLowerCase().includes(forbiddenRuntimeReference.toLowerCase()),
    false,
    `release source must not include ${forbiddenRuntimeReference}`
  );
}

assert.equal(releaseSource.includes("http://localhost:3000/health"), true);
assert.equal(releaseSource.includes(`>${manifest.version}<`), true);
assert.equal(releaseSource.includes("observedExternalUri"), true);
assert.equal(releaseSource.includes("displayUrl"), false);
assert.equal(releaseSource.includes("<img"), false);
assert.equal(releaseSource.includes("background-image"), false);

const partialPackage = buildFirstDiscoveryPackage({
  observedAt: "2026-08-25T10:00:00+08:00",
  stoppedReason: "manual_stop",
  cards: [
    {
      platformContentId: "xhs-note-1",
      title: "A娃作业启动",
      creatorDisplayName: "公开展示名",
      publishedAtSourceText: "3小时前",
      coverCandidate: { observedExternalUri: "https://observed.example.invalid/cover.jpg" },
      resultPosition: 1
    },
    {
      platformContentId: "xhs-note-2",
      resultPosition: 2
    }
  ]
});

assert.equal(partialPackage.contractVersion, FIRST_DISCOVERY_CONTRACT_VERSION);
assert.equal(partialPackage.coverage.visibleCards, 2);
assert.equal(partialPackage.coverage.stoppedReason, "manual_stop");
assert.equal(partialPackage.cards[0].occurrence.resultPosition, 1);
assert.equal("displayUrl" in partialPackage.cards[0].content.coverCandidate, false);
assert.equal("body" in partialPackage.cards[0].content, false);
assert.throws(
  () =>
    buildFirstDiscoveryPackage({
      observedAt: "2026-08-25T10:00:00+08:00",
      stoppedReason: "manual_stop",
      cards: [{ platformContentId: "xhs-note-1", body: "not discovery", resultPosition: 1 }]
    }),
  /may not contain body/
);
assert.throws(
  () =>
    buildFirstDiscoveryPackage({
      observedAt: "2026-08-25T10:00:00+08:00",
      stoppedReason: "manual_stop",
      cards: [
        { platformContentId: "xhs-note-1", resultPosition: 1 },
        { platformContentId: "xhs-note-2", resultPosition: 1 }
      ]
    }),
  /result positions must be unique/
);
assert.throws(
  () => buildFirstDiscoveryPackage({ observedAt: "2026-08-25T10:00:00", stoppedReason: "manual_stop", cards: [] }),
  /RFC 3339/
);
assert.throws(
  () => buildFirstDiscoveryPackage({ observedAt: "2026-02-30T10:00:00Z", stoppedReason: "manual_stop", cards: [] }),
  /RFC 3339/
);

const archiveListing = execFileSync(
  "unzip",
  ["-l", join(packageDirectory, "releases", "linggan-browser-producer-0.1.0.zip")],
  { encoding: "utf8" }
);
for (const requiredReleaseFile of [
  "manifest.json",
  "service-worker.js",
  "discovery-contract.js",
  "popup.html",
  "popup.css",
  "popup.js",
  "lids-tokens.css"
]) {
  assert.equal(archiveListing.includes(requiredReleaseFile), true, `${requiredReleaseFile} missing from ZIP`);
}

process.stdout.write("Linggan Browser Producer static and contract checks passed.\n");
