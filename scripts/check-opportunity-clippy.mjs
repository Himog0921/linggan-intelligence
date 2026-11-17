#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { isAbsolute, relative, resolve, sep } from 'node:path';

const root = resolve(fileURLToPath(new URL('..', import.meta.url)));
const targeted = [
  /^crates\/intelligence\/src\/opportunity(?:\.rs$|\/)/,
  /^crates\/intelligence\/src\/ai_foundation\/executor\/service\/opportunity_sync\.rs$/,
  /^crates\/intelligence\/tests\/opportunity_[^/]*(?:\/|$)/,
  /^crates\/intelligence\/tests\/support\/opportunity_dataset\.rs$/,
  /^apps\/api\/src\/local_web\/opportunity_api\.rs$/,
  /^apps\/worker\/src\/bin\/opportunity\.rs$/,
];

export function classify(message) {
  if (message?.reason !== 'compiler-message' || !['warning', 'error'].includes(message.message?.level)) return [];
  const diagnostic = message.message;
  return (diagnostic.spans ?? []).filter(span => span.is_primary).map(span => {
    const path = (isAbsolute(span.file_name) ? relative(root, span.file_name) : span.file_name).split(sep).join('/');
    return { path, line: span.line_start, lint: diagnostic.code?.code ?? diagnostic.level, target: targeted.some(pattern => pattern.test(path)) };
  });
}

export async function run() {
  const env = { ...process.env, RUSTFLAGS: [process.env.RUSTFLAGS, '--cap-lints=warn'].filter(Boolean).join(' ') };
  if (process.env.CARGO_ENCODED_RUSTFLAGS) env.CARGO_ENCODED_RUSTFLAGS += '\x1f--cap-lints=warn';
  const child = spawn('cargo', ['clippy', '-p', 'linggan-intelligence', '-p', 'linggan-worker', '-p', 'linggan-api', '--all-targets', '--locked', '--message-format=json'], { cwd: root, env, stdio: ['ignore', 'pipe', 'inherit'] });
  const completed = new Promise((resolveExit, reject) => { child.once('error', reject); child.once('close', resolveExit); });
  let other = 0;
  let relevant = 0;
  for await (const line of createInterface({ input: child.stdout })) {
    let value;
    try { value = JSON.parse(line); } catch { continue; }
    if (value?.reason !== 'compiler-message' || !['warning', 'error'].includes(value.message?.level)) continue;
    const matches = classify(value).filter(item => item.target);
    if (matches.length) {
      relevant++;
      for (const item of matches) console.error(`${item.path}:${item.line} ${item.lint}`);
    } else {
      other++;
    }
  }
  const exit = await completed;
  console.error(`Opportunity diagnostics: ${relevant}; other diagnostics: ${other}; cargo exit: ${exit}`);
  return exit === 0 && relevant === 0 ? 0 : 1;
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  process.exitCode = await run();
}
