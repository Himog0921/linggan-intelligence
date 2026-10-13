import {homedir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';

// Build tooling only. The delivered page has no dependency on this installation.
const root=process.env.ANATOMY_SKILL_ROOT||join(homedir(),'.codex/skills/anatomy');
export const k=await import(pathToFileURL(join(root,'kit/iso-kit.mjs')));
export const A=await import(pathToFileURL(join(root,'kit/audit.mjs')));
