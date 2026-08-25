import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  unlinkSync,
  utimesSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageDirectory = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const projectDirectory = resolve(packageDirectory, "../..");
const sourceDirectory = join(packageDirectory, "src");
const runtimeTokens = join(projectDirectory, "apps/api/src/local_web/lids_tokens.css");
const releaseDirectory = join(packageDirectory, "releases");
const releaseManifestPath = join(releaseDirectory, "release-manifest.json");
const fixedBuildTime = new Date("2026-08-25T00:00:00.000Z");
const sourceFiles = [
  "manifest.json",
  "service-worker.js",
  "discovery-contract.js",
  "xhs-visible-search-adapter.js",
  "popup.html",
  "popup.css",
  "popup.js"
];

function sha256File(filePath) {
  return createHash("sha256").update(readFileSync(filePath)).digest("hex");
}

function recursivelyListFiles(directory, root = directory) {
  return readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {
        return recursivelyListFiles(path, root);
      }
      return [relative(root, path)];
    })
    .sort();
}

function normalizeBuildTimestamp(directory) {
  for (const file of recursivelyListFiles(directory)) {
    const path = join(directory, file);
    utimesSync(path, fixedBuildTime, fixedBuildTime);
  }
}

function readPackageVersion() {
  const manifest = JSON.parse(readFileSync(join(sourceDirectory, "manifest.json"), "utf8"));
  if (typeof manifest.version !== "string" || manifest.version.trim() === "") {
    throw new Error("Browser Producer manifest must have a non-empty version.");
  }
  return manifest.version;
}

function buildInto(outputDirectory) {
  mkdirSync(outputDirectory, { recursive: true });

  for (const file of sourceFiles) {
    copyFileSync(join(sourceDirectory, file), join(outputDirectory, file));
  }
  copyFileSync(runtimeTokens, join(outputDirectory, "lids-tokens.css"));
  normalizeBuildTimestamp(outputDirectory);

  const version = readPackageVersion();
  const archivePath = join(dirname(outputDirectory), `linggan-browser-producer-${version}.zip`);
  if (existsSync(archivePath)) {
    unlinkSync(archivePath);
  }

  execFileSync(
    "zip",
    ["-X", "-q", archivePath, ...recursivelyListFiles(outputDirectory)],
    { cwd: outputDirectory }
  );

  const files = recursivelyListFiles(outputDirectory).map((file) => ({
    path: file,
    sha256: sha256File(join(outputDirectory, file)),
    bytes: statSync(join(outputDirectory, file)).size
  }));

  return {
    archivePath,
    releaseManifest: {
      format: "linggan-browser-producer-release-v1",
      package: "linggan-browser-producer",
      version,
      contractCompatibility: ["xhs.discovery.visible-card.v1"],
      loopbackHealthTarget: "http://localhost:3000/health",
      archive: {
        file: basename(archivePath),
        sha256: sha256File(archivePath),
        bytes: statSync(archivePath).size
      },
      files
    }
  };
}

function writeRelease() {
  const distributionDirectory = join(packageDirectory, "dist");
  rmSync(distributionDirectory, { recursive: true, force: true });
  mkdirSync(releaseDirectory, { recursive: true });

  const output = buildInto(distributionDirectory);
  const releaseArchive = join(releaseDirectory, basename(output.archivePath));
  copyFileSync(output.archivePath, releaseArchive);
  writeFileSync(releaseManifestPath, `${JSON.stringify(output.releaseManifest, null, 2)}\n`);
  rmSync(output.archivePath, { force: true });

  process.stdout.write(`Built ${basename(releaseArchive)}\n`);
  process.stdout.write(`SHA-256 ${output.releaseManifest.archive.sha256}\n`);
}

function checkRelease() {
  if (!existsSync(releaseManifestPath)) {
    throw new Error("Release manifest is missing; run npm run build.");
  }

  const temporaryDirectory = mkdtempSync(join(tmpdir(), "linggan-browser-producer-check-"));
  try {
    const output = buildInto(join(temporaryDirectory, "dist"));
    const committed = JSON.parse(readFileSync(releaseManifestPath, "utf8"));
    const committedArchivePath = join(releaseDirectory, committed.archive.file);
    if (!existsSync(committedArchivePath)) {
      throw new Error(`Committed release archive is missing: ${committed.archive.file}`);
    }
    if (JSON.stringify(committed) !== JSON.stringify(output.releaseManifest)) {
      throw new Error("Release manifest does not match a clean rebuild.");
    }
    if (sha256File(committedArchivePath) !== output.releaseManifest.archive.sha256) {
      throw new Error("Committed release archive does not match a clean rebuild.");
    }
    process.stdout.write(`Release check passed: ${committed.archive.file}\n`);
  } finally {
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
}

if (process.argv.includes("--check")) {
  checkRelease();
} else {
  writeRelease();
}
