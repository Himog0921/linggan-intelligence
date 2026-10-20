import type { AiSettings, Json } from './aiSettingsModel';

export function readProtocolFields(form: FormData, spec: AiSettings['protocol']['apis'][string]) {
  const compat: Record<string, Json> = {}; const thinkingLevelMap: Record<string, string> = {};
  for (const [key, value] of form.entries()) {
    if (key.startsWith('compat:') && value !== '') {
      const name = key.slice(7); compat[name] = spec.booleans.includes(name) ? value === 'true' : Object.hasOwn(spec.numbers, name) ? Number(value) : String(value);
    }
    if (key.startsWith('thinking:') && value !== '') thinkingLevelMap[key.slice(9)] = String(value);
    const match = /^template:([^:]+):(\d+):key$/.exec(key);
    if (!match) continue;
    const name = match[1]!; const prefix = `template:${name}:${match[2]}:`; const property = String(value);
    if (!property || ['__proto__', 'prototype', 'constructor'].includes(property)) throw new Error('扩展参数名不合格。');
    const current = compat[name]; const template = current && typeof current === 'object' && !Array.isArray(current) ? current : {};
    if (Object.hasOwn(template, property)) throw new Error(`扩展参数 ${property} 重复。`);
    const kind = String(form.get(`${prefix}kind`)); const fixed = String(form.get(`${prefix}value`) ?? '');
    if (kind.startsWith('thinking.')) template[property] = { $var: kind, omitWhenOff: form.get(`${prefix}omit`) === 'on' };
    else if (kind === 'boolean') { if (fixed !== 'true' && fixed !== 'false') throw new Error(`${property}：布尔值只能填写 true 或 false。`); template[property] = fixed === 'true'; }
    else if (kind === 'number') { if (!fixed.trim() || !Number.isFinite(Number(fixed))) throw new Error(`${property}：请输入有限数值。`); template[property] = Number(fixed); }
    else template[property] = fixed;
    compat[name] = template;
  }
  return { compat, thinkingLevelMap };
}
