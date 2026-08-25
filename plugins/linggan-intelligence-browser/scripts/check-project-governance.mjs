#!/usr/bin/env node

import { existsSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const failures = [];

function read(relativePath) {
  return readFileSync(path.join(rootDir, relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function expectPathExists(relativePath) {
  if (!existsSync(path.join(rootDir, relativePath))) {
    fail(`required file is missing: ${relativePath}`);
  }
}

function expectIncludes(relativePath, expected) {
  const content = read(relativePath);
  if (!content.includes(expected)) {
    fail(`${relativePath} missing expected text: ${expected}`);
  }
}

function checkRootMarkdownPlacement() {
  const allowedRootMarkdown = new Set(["AGENTS.md", "CLAUDE.md"]);
  const unexpected = readdirSync(rootDir)
    .filter((name) => name.endsWith(".md"))
    .filter((name) => !allowedRootMarkdown.has(name));

  if (unexpected.length > 0) {
    fail(`unexpected root markdown files: ${unexpected.join(", ")}`);
  }
}

function checkGovernanceDocs() {
  [
    "docs/governance/file-placement-standard.md",
    "docs/governance/document-maintenance-protocol.md",
  ].forEach(expectPathExists);

  ["AGENTS.md", "CLAUDE.md"].forEach((relativePath) => {
    expectIncludes(relativePath, "docs/governance/file-placement-standard.md");
    expectIncludes(relativePath, "docs/governance/document-maintenance-protocol.md");
  });

  expectIncludes("docs/README.md", "governance/file-placement-standard.md");
  expectIncludes("docs/README.md", "governance/document-maintenance-protocol.md");
}

function checkVersionDocs() {
  const packageVersion = JSON.parse(read("package.json")).version;
  const manifestVersion = JSON.parse(read("manifest.json")).version;

  if (packageVersion !== manifestVersion) {
    fail(`package.json version (${packageVersion}) does not match manifest.json version (${manifestVersion})`);
  }

  expectIncludes("docs/README.md", `v${packageVersion}`);
  expectIncludes("docs/chrome-web-store/README.md", `Version: ${packageVersion}`);
  expectIncludes("docs/chrome-web-store/README.md", `linggan-boom-v${packageVersion}.zip`);
}

function checkLocalLinks(relativePath) {
  const content = read(relativePath);
  const dir = path.dirname(path.join(rootDir, relativePath));
  const linkPattern = /\[[^\]]+\]\(([^)]+)\)/g;
  let match;

  while ((match = linkPattern.exec(content))) {
    const rawTarget = match[1].trim();
    if (
      !rawTarget ||
      rawTarget.startsWith("#") ||
      rawTarget.startsWith("http://") ||
      rawTarget.startsWith("https://") ||
      rawTarget.startsWith("mailto:") ||
      rawTarget.startsWith("/Users/")
    ) {
      continue;
    }

    const target = rawTarget.split("#")[0];
    if (!target) continue;

    const resolved = path.resolve(dir, target);
    if (!existsSync(resolved)) {
      fail(`${relativePath} has broken local link: ${rawTarget}`);
    }
  }
}

function checkCoreLinks() {
  [
    "AGENTS.md",
    "CLAUDE.md",
    "docs/README.md",
    "docs/chrome-web-store/README.md",
  ].forEach(checkLocalLinks);
}

checkRootMarkdownPlacement();
checkGovernanceDocs();
checkVersionDocs();
checkCoreLinks();

if (failures.length === 0) {
  process.stdout.write("Project governance check passed.\n");
} else {
  process.stderr.write(`Project governance check failed:\n${failures.map((item) => `- ${item}`).join("\n")}\n`);
}

process.exit(failures.length === 0 ? 0 : 1);
