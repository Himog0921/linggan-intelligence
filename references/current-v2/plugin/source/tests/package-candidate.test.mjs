import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

import JSZip from "jszip";

import {
  assertRepositoryStateUnchanged,
  buildCandidatePackage,
  resolveAllowedOutputDirectory,
  snapshotRepositoryState,
} from "../scripts/package-candidate.mjs";

const RELEASE_CANDIDATE_VERSION = "2.0.95";

function tempDir(label) {
  return mkdtempSync(join("/tmp", `${label}-`));
}

function git(root, args) {
  return execFileSync("git", args, { cwd: root, encoding: "utf8" });
}

function commitFixture(root) {
  git(root, ["-c", "user.name=Candidate Test", "-c", "user.email=candidate@example.invalid", "commit", "-qm", "fixture"]);
}

function syntheticRepository() {
  const root = tempDir("candidate-source");
  git(root, ["init", "-q"]);
  writeFileSync(join(root, ".gitignore"), "dist/\n");
  writeFileSync(join(root, "manifest.json"), '{"version":"9.9.9","manifest_version":3}\n');
  writeFileSync(join(root, "tracked.txt"), "tracked WIP\n");
  git(root, ["add", ".gitignore", "manifest.json", "tracked.txt"]);
  commitFixture(root);
  writeFileSync(join(root, "tracked.txt"), "tracked candidate WIP\n");
  mkdirSync(join(root, "dist", "nested"), { recursive: true });
  writeFileSync(join(root, "dist", "manifest.json"), readFileSync(join(root, "manifest.json")));
  writeFileSync(join(root, "dist", "background.js"), "console.log('candidate');\n");
  writeFileSync(join(root, "dist", "nested", "asset.txt"), "asset\n");
  return root;
}

test("release candidate has one distinguishable version across authoritative version files", () => {
  const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
  const packageLock = JSON.parse(readFileSync(new URL("../package-lock.json", import.meta.url), "utf8"));
  const manifest = JSON.parse(readFileSync(new URL("../manifest.json", import.meta.url), "utf8"));

  assert.equal(packageJson.version, RELEASE_CANDIDATE_VERSION);
  assert.equal(packageLock.version, RELEASE_CANDIDATE_VERSION);
  assert.equal(packageLock.packages[""].version, RELEASE_CANDIDATE_VERSION);
  assert.equal(manifest.version, RELEASE_CANDIDATE_VERSION);
});

test("output allowlist rejects paths outside /tmp and symlink escapes", () => {
  const output = tempDir("candidate-output");
  assert.equal(resolveAllowedOutputDirectory(output), realpathSync(output));
  assert.throws(() => resolveAllowedOutputDirectory("/candidate-output"), /inside \/tmp/);

  const linkParent = tempDir("candidate-link-parent");
  const outside = "/";
  const link = join(linkParent, "escape");
  symlinkSync(outside, link);
  assert.throws(() => resolveAllowedOutputDirectory(join(link, "candidate")), /resolves outside/);
});

test("repository snapshot detects tracked WIP byte changes even when porcelain status is unchanged", () => {
  const root = tempDir("candidate-state");
  git(root, ["init", "-q"]);
  writeFileSync(join(root, "tracked.txt"), "indexed\n");
  git(root, ["add", "tracked.txt"]);
  commitFixture(root);
  writeFileSync(join(root, "tracked.txt"), "WIP-A\n");
  const before = snapshotRepositoryState(root);
  writeFileSync(join(root, "tracked.txt"), "WIP-B\n");
  const after = snapshotRepositoryState(root);

  assert.equal(before.statusPorcelain, after.statusPorcelain);
  assert.notEqual(before.trackedWorktreeSha256, after.trackedWorktreeSha256);
  assert.throws(() => assertRepositoryStateUnchanged(before, after), /tracked working-tree blobs/);
});

test("candidate package is side-effect-free and its content manifest matches every zip entry", async () => {
  const root = syntheticRepository();
  const outputDir = tempDir("candidate-artifacts");
  const before = snapshotRepositoryState(root);
  const result = await buildCandidatePackage({ root, outputDir });
  const after = snapshotRepositoryState(root);
  assertRepositoryStateUnchanged(before, after);

  assert.match(result.packagePath, /^\/private\/tmp\/|^\/tmp\//);
  const zipBytes = readFileSync(result.packagePath);
  const contents = JSON.parse(readFileSync(result.contentsPath, "utf8"));
  assert.equal(contents.package.sha256, result.sha256);
  assert.equal(contents.package.size, zipBytes.length);
  assert.equal(contents.source.trackedWorktreeSha256, before.trackedWorktreeSha256);
  assert.deepEqual(contents.files.map(({ path }) => path), [
    "background.js",
    "manifest.json",
    "nested/asset.txt",
  ]);

  const zip = await JSZip.loadAsync(zipBytes);
  assert.deepEqual(Object.keys(zip.files).sort(), contents.files.map(({ path }) => path));
  for (const entry of contents.files) {
    assert.deepEqual(await zip.file(entry.path).async("nodebuffer"), readFileSync(join(root, "dist", entry.path)));
  }
  assert.equal(readFileSync(result.sidecarPath, "utf8"), `${result.sha256}  ${result.packagePath.split("/").pop()}\n`);
});
