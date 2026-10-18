#!/usr/bin/env node
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

function files(directory) { return readdirSync(directory, { recursive: true, encoding: 'utf8' }).map(name => path.join(directory, name)); }
export function violations(root) {
  const api = path.join(root, 'apps/api/src'), failures = [];
  for (const file of files(api)) {
    const name = path.relative(root, file);
    if (/\.(html|css|js|mjs)$/.test(file)) failures.push(`${name}: production UI assets belong to apps/web`);
    if (!file.endsWith('.rs') || /(?:^tests|_tests|_fixture|_private_review)\.rs$/.test(path.basename(file))) continue;
    const source = readFileSync(file, 'utf8');
    if (file !== path.join(api, 'local_web/web_app.rs') && (/\bHtml\s*\(/.test(source) || /<(?:!doctype|html|body|main|section|header|nav|form|button|div)\b/i.test((source.match(/"(?:\\.|[^"\\])*"/g) ?? []).join("\n")))) failures.push(`${name}: Rust business page HTML is forbidden`);
    if (/include_str!\([^\n]*\.(?:html|css|js)["']/.test(source)) failures.push(`${name}: Rust embeds a page asset`);
  }
  const web = path.join(root, 'apps/web/src');
  const roots = files(web).filter(file => /\.(?:tsx?|jsx?)$/.test(file) && !/\.test\./.test(file)).flatMap(file => [...readFileSync(file, 'utf8').matchAll(/\b(?:createRoot|hydrateRoot)\s*\(/g)].map(() => file));
  if (roots.length !== 1 || roots[0] !== path.join(web, 'main.tsx')) failures.push('apps/web must have exactly one React root, in src/main.tsx');
  return failures;
}
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  const failures = violations(root);
  if (failures.length) { console.error(failures.join('\n')); process.exitCode = 1; }
  else console.log('React boundary PASS: one root; no Rust business pages or old UI assets.');
}
