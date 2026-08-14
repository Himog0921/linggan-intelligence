import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import JSZip from 'jszip';

function readJson(file) {
  return JSON.parse(readFileSync(file, 'utf8'));
}

function readArg(name, fallback = '') {
  const index = process.argv.indexOf(name);
  if (index === -1) return fallback;
  return process.argv[index + 1] || fallback;
}

function assertCondition(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertIncludesAll(values = [], required = [], label = '') {
  for (const value of required) {
    assertCondition(
      values.includes(value),
      `${label} is missing ${value}`,
    );
  }
}

function assertDouyinManifestContract(manifest = {}, label = 'manifest') {
  const douyinContentScript = (manifest.content_scripts || []).find((entry) =>
    (entry.matches || []).includes('https://www.douyin.com/*')
  );
  assertCondition(douyinContentScript, `${label} is missing douyin content script`);
  assertIncludesAll(
    douyinContentScript.js || [],
    ['vendor.js', 'content.js'],
    `${label} douyin content script js`,
  );
  assertIncludesAll(
    douyinContentScript.css || [],
    ['content.css'],
    `${label} douyin content script css`,
  );

  assertIncludesAll(
    manifest.host_permissions || [],
    [
      'https://www.douyin.com/*',
      'https://*.douyinpic.com/*',
      'https://*.douyinvod.com/*',
    ],
    `${label} host_permissions`,
  );

  const douyinResources = (manifest.web_accessible_resources || []).find((entry) =>
    (entry.matches || []).includes('https://www.douyin.com/*')
  );
  assertCondition(douyinResources, `${label} is missing douyin web accessible resources`);
  assertIncludesAll(
    douyinResources.resources || [],
    ['injected/douyinApiCapture.js'],
    `${label} douyin web accessible resources`,
  );
}

const packageJson = readJson('package.json');
const expectedVersion = readArg('--version', packageJson.version);
const zipPath = readArg('--zip', path.join('releases', `linggan-boom-v${expectedVersion}.zip`));

const manifestJson = readJson('manifest.json');
const packageLockJson = readJson('package-lock.json');
const distManifestPath = path.join('dist', 'manifest.json');

assertCondition(packageJson.version === expectedVersion, `package.json version is ${packageJson.version}, expected ${expectedVersion}`);
assertCondition(packageLockJson.version === expectedVersion, `package-lock.json version is ${packageLockJson.version}, expected ${expectedVersion}`);
assertCondition(packageLockJson.packages?.['']?.version === expectedVersion, `package-lock root package version is ${packageLockJson.packages?.['']?.version}, expected ${expectedVersion}`);
assertCondition(manifestJson.version === expectedVersion, `manifest.json version is ${manifestJson.version}, expected ${expectedVersion}`);
assertDouyinManifestContract(manifestJson, 'manifest.json');
assertCondition(existsSync(distManifestPath), 'dist/manifest.json is missing; run npm run build first');

const distManifestJson = readJson(distManifestPath);
assertCondition(distManifestJson.version === expectedVersion, `dist/manifest.json version is ${distManifestJson.version}, expected ${expectedVersion}`);
assertDouyinManifestContract(distManifestJson, 'dist/manifest.json');

const requiredDistFiles = [
  'manifest.json',
  'background.js',
  'content.js',
  'content.css',
  'vendor.js',
  'popup.js',
  'dashboard.js',
  'popup.html',
  'dashboard.html',
  'injected/douyinApiCapture.js',
];

for (const file of requiredDistFiles) {
  assertCondition(existsSync(path.join('dist', file)), `dist/${file} is missing`);
}

assertCondition(existsSync(zipPath), `${zipPath} is missing`);

const zip = await JSZip.loadAsync(readFileSync(zipPath));
for (const file of requiredDistFiles) {
  const zippedFile = zip.file(file);
  assertCondition(zippedFile, `${zipPath} is missing ${file}`);

  const distBytes = readFileSync(path.join('dist', file));
  const zippedBytes = await zippedFile.async('nodebuffer');
  assertCondition(
    Buffer.compare(distBytes, zippedBytes) === 0,
    `${zipPath} ${file} does not match dist/${file}; rebuild the release zip after npm run build`,
  );
}

const zipManifest = JSON.parse(await zip.file('manifest.json').async('string'));
assertCondition(zipManifest.version === expectedVersion, `${zipPath} manifest version is ${zipManifest.version}, expected ${expectedVersion}`);
assertDouyinManifestContract(zipManifest, `${zipPath} manifest.json`);

console.log(`release package verified: v${expectedVersion} -> ${zipPath}`);
