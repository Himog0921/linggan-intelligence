/**
 * V2 XHS Content-only Release Candidate — side-effect-free package builder.
 *
 * The archive is built only from dist/. Packaging is allowed to write only
 * below /tmp or /private/tmp and must leave both Git status and every tracked
 * working-tree blob byte-identical to their pre-package state.
 */

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  readlinkSync,
  realpathSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import JSZip from "jszip";

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const DEFAULT_ROOT = resolve(dirname(SCRIPT_PATH), "..");
const ZIP_ENTRY_DATE = new Date("1980-01-01T00:00:00.000Z");
const OUTPUT_ROOTS = ["/tmp", "/private/tmp"];

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function isWithin(root, candidate) {
  const remainder = relative(root, candidate);
  return remainder === "" || (!remainder.startsWith(`..${sep}`) && remainder !== "..");
}

function nearestExistingAncestor(pathname) {
  let current = pathname;
  const suffix = [];
  while (!existsSync(current)) {
    const parent = dirname(current);
    if (parent === current) throw new Error(`No existing ancestor for ${pathname}`);
    suffix.unshift(basename(current));
    current = parent;
  }
  return { ancestor: current, suffix };
}

/** Resolve the output directory without allowing a symlink escape from /tmp. */
export function resolveAllowedOutputDirectory(outputDir) {
  if (typeof outputDir !== "string" || outputDir.trim() === "") {
    throw new Error("Candidate output directory is required.");
  }

  const requested = resolve(outputDir);
  if (!OUTPUT_ROOTS.some((root) => isWithin(root, requested))) {
    throw new Error("Candidate output must be inside /tmp or /private/tmp.");
  }

  const canonicalRoots = [...new Set(OUTPUT_ROOTS.map((root) => realpathSync(root)))];
  const { ancestor, suffix } = nearestExistingAncestor(requested);
  const canonicalCandidate = resolve(realpathSync(ancestor), ...suffix);
  if (!canonicalRoots.some((root) => isWithin(root, canonicalCandidate))) {
    throw new Error("Candidate output resolves outside /tmp or /private/tmp.");
  }

  mkdirSync(canonicalCandidate, { recursive: true });
  const finalPath = realpathSync(canonicalCandidate);
  if (!canonicalRoots.some((root) => isWithin(root, finalPath))) {
    throw new Error("Candidate output resolves outside /tmp or /private/tmp.");
  }
  return finalPath;
}

function git(root, args, options = {}) {
  return execFileSync("git", args, {
    cwd: root,
    encoding: options.encoding ?? "utf8",
    maxBuffer: 16 * 1024 * 1024,
  });
}

function trackedFileFingerprint(root, path) {
  const absolute = join(root, path);
  if (!existsSync(absolute)) return { path, kind: "missing", sha256: null };
  const stat = lstatSync(absolute);
  if (stat.isSymbolicLink()) {
    return {
      path,
      kind: "symlink",
      sha256: sha256(Buffer.from(readlinkSync(absolute))),
    };
  }
  if (!stat.isFile()) return { path, kind: "other", sha256: null };
  return { path, kind: "file", sha256: sha256(readFileSync(absolute)) };
}

/** Snapshot both porcelain state and actual current bytes of every tracked path. */
export function snapshotRepositoryState(root) {
  const trackedPaths = git(root, ["ls-files", "-z"], { encoding: "buffer" })
    .toString("utf8")
    .split("\0")
    .filter(Boolean)
    .sort();
  const trackedFiles = trackedPaths.map((path) => trackedFileFingerprint(root, path));
  const statusPorcelain = git(root, ["status", "--porcelain=v1", "-z"]);
  return {
    head: git(root, ["rev-parse", "HEAD"]).trim(),
    statusPorcelain,
    statusSha256: sha256(Buffer.from(statusPorcelain)),
    trackedFiles,
    trackedWorktreeSha256: sha256(Buffer.from(JSON.stringify(trackedFiles))),
  };
}

export function assertRepositoryStateUnchanged(before, after) {
  if (before.statusPorcelain !== after.statusPorcelain) {
    throw new Error("Candidate packaging changed Git working-tree status.");
  }
  if (before.trackedWorktreeSha256 !== after.trackedWorktreeSha256) {
    throw new Error("Candidate packaging changed one or more tracked working-tree blobs.");
  }
  if (before.head !== after.head) {
    throw new Error("Candidate packaging changed HEAD.");
  }
}

function listDistFiles(dist, current = "") {
  const absolute = join(dist, current);
  const files = [];
  for (const name of readdirSync(absolute).sort()) {
    const path = current ? `${current}/${name}` : name;
    const full = join(dist, path);
    const stat = statSync(full);
    if (stat.isDirectory()) files.push(...listDistFiles(dist, path));
    else if (stat.isFile()) files.push(path);
    else throw new Error(`Unsupported dist entry: ${path}`);
  }
  return files;
}

function writeAtomic(path, bytes) {
  const stagingDir = mkdtempSync(join(dirname(path), ".candidate-staging-"));
  const staged = join(stagingDir, basename(path));
  try {
    writeFileSync(staged, bytes, { flag: "wx" });
    renameSync(staged, path);
  } finally {
    rmSync(stagingDir, { recursive: true, force: true });
  }
}

export async function buildCandidatePackage({ root = DEFAULT_ROOT, outputDir }) {
  const resolvedRoot = realpathSync(root);
  const out = resolveAllowedOutputDirectory(outputDir);
  const dist = join(resolvedRoot, "dist");
  const manifestPath = join(resolvedRoot, "manifest.json");
  const distManifestPath = join(dist, "manifest.json");
  const before = snapshotRepositoryState(resolvedRoot);

  if (!existsSync(dist)) throw new Error("dist/ not found. Run `npm run build` first.");
  if (!existsSync(manifestPath) || !existsSync(distManifestPath)) {
    throw new Error("manifest.json must exist in both the repository root and dist/.");
  }
  const rootManifestBytes = readFileSync(manifestPath);
  const distManifestBytes = readFileSync(distManifestPath);
  if (!rootManifestBytes.equals(distManifestBytes)) {
    throw new Error("dist/manifest.json does not byte-match tracked manifest.json; rebuild first.");
  }

  const manifest = JSON.parse(rootManifestBytes.toString("utf8"));
  if (typeof manifest.version !== "string" || manifest.version.trim() === "") {
    throw new Error("manifest.json missing version field.");
  }

  const files = listDistFiles(dist);
  if (files.length === 0) throw new Error("dist/ contains no files.");
  const zip = new JSZip();
  const contents = [];
  for (const path of files) {
    const bytes = readFileSync(join(dist, path));
    contents.push({ path, size: bytes.length, sha256: sha256(bytes) });
    zip.file(path, bytes, { date: ZIP_ENTRY_DATE, createFolders: false });
  }

  const zipBuffer = await zip.generateAsync({
    type: "nodebuffer",
    compression: "DEFLATE",
    compressionOptions: { level: 9 },
    platform: "UNIX",
  });
  const packageSha256 = sha256(zipBuffer);
  const filename = `linggan-boom-v2-xhs-content-rc-${manifest.version}-${packageSha256.slice(0, 8)}.zip`;
  const packagePath = join(out, filename);
  const sidecarPath = `${packagePath}.sha256`;
  const contentsPath = `${packagePath}.contents.json`;
  const candidateManifest = {
    schemaVersion: 1,
    package: { filename, sha256: packageSha256, size: zipBuffer.length },
    source: {
      head: before.head,
      statusSha256: before.statusSha256,
      trackedWorktreeSha256: before.trackedWorktreeSha256,
    },
    files: contents,
  };

  writeAtomic(packagePath, zipBuffer);
  writeAtomic(sidecarPath, Buffer.from(`${packageSha256}  ${filename}\n`));
  writeAtomic(contentsPath, Buffer.from(`${JSON.stringify(candidateManifest, null, 2)}\n`));

  const after = snapshotRepositoryState(resolvedRoot);
  try {
    assertRepositoryStateUnchanged(before, after);
  } catch (error) {
    rmSync(packagePath, { force: true });
    rmSync(sidecarPath, { force: true });
    rmSync(contentsPath, { force: true });
    throw error;
  }

  return {
    packagePath,
    sidecarPath,
    contentsPath,
    sha256: packageSha256,
    size: zipBuffer.length,
    fileCount: contents.length,
    source: candidateManifest.source,
  };
}

function readArg(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? undefined : process.argv[index + 1];
}

async function main() {
  const outputDir = readArg("--output");
  if (!outputDir) throw new Error("Usage: node scripts/package-candidate.mjs --output <dir>");
  const result = await buildCandidatePackage({ outputDir });
  console.log(`Candidate package: ${result.packagePath}`);
  console.log(`  SHA-256: ${result.sha256}`);
  console.log(`  Size:    ${result.size} bytes`);
  console.log(`  Files:   ${result.fileCount}`);
  console.log(`  Contents: ${result.contentsPath}`);
  console.log(`  Sidecar: ${result.sidecarPath}`);
  console.log(`  Source tracked-worktree SHA-256: ${result.source.trackedWorktreeSha256}`);
  console.log("Git status and tracked working-tree blobs are byte-identical before/after packaging. ✅");
}

if (process.argv[1] && resolve(process.argv[1]) === SCRIPT_PATH) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
