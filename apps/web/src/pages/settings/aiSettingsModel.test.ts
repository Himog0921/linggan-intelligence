import { describe, expect, it } from 'vitest';
import { ApiError } from '../../shared/api';
import { controlReason, controlStatus, count, parseAiSettings, parseSamples } from './aiSettingsModel';
import { settingsFixture } from './aiSettingsFixture.test-support';
import { readSkillFolder } from './aiSkillUpload';
import { readProtocolFields } from './aiProtocolForm';

describe('AI settings boundary', () => {
  it('accepts the full contract and preserves current-key unknown versus stable selection', () => {
    const data = parseAiSettings(settingsFixture());
    expect(data.models[0]).toMatchObject({ selectionModelRef: 'old-model-ref', modelRef: null, probe: null });
    expect(data.execution.lateUsage.knownInputTokens).toBeNull();
  });
  it('rejects missing fields, inherited keys, unsafe numeric counts and another contract', () => {
    const data = settingsFixture();
    for (const value of [{ ...data, contract: 'ai-settings.future' }, { ...data, execution: null },
      { ...data, resources: { ...data.resources, maxConcurrency: Number.MAX_SAFE_INTEGER + 1 } },
      { ...data, constructor: 'unexpected' }, { ...data, models: [{ ...data.models[0], selectionModelRef: undefined }] }]) {
      expect(() => parseAiSettings(value)).toThrow(ApiError);
    }
  });
  it('retains NULL, known zero and large unsigned decimal counts', () => {
    expect(count(null)).toBe('—'); expect(count('0')).toBe('0'); expect(count('9007199254740993')).toBe('9,007,199,254,740,993');
    expect(() => parseSamples({ state: 'connected', samples: [], examined: '0', limit: 50 })).toThrow(ApiError);
  });
  it('keeps initial off, manual pause and actual automatic brake separate', () => {
    const control = parseAiSettings(settingsFixture()).controls[0]!;
    expect(controlStatus({ ...control, actor: 'system', reason: 'not_enabled' })).toBe('初始未启用');
    expect(controlReason({ ...control, actor: 'system', reason: 'not_enabled' })).toBe('尚无人启用');
    expect(controlReason({ ...control, actor: 'human', reason: 'automatic_brake:failure_ratio' })).toBe('automatic_brake:failure_ratio');
    expect(controlStatus({ ...control, commandRef: 'command', revision: 1 })).toBe('人工暂停');
    const braked = { ...control, commandRef: 'round', revision: 1, actor: 'system', reason: 'automatic_brake:failure_ratio' };
    expect(controlStatus(braked)).toBe('系统自动暂停'); expect(controlReason(braked)).toBe('本轮失败比例严格超过 30%');
  });
  it('structured compatibility editor preserves explicit false, zero, variable references and mapping', () => {
    const form = new FormData();
    form.set('compat:supportsDeveloperRole', 'false'); form.set('compat:vllmPriority', '0');
    form.set('thinking:high', 'custom-high');
    form.set('template:chatTemplateKwargs:0:key', 'reasoning'); form.set('template:chatTemplateKwargs:0:kind', 'thinking.enabled'); form.set('template:chatTemplateKwargs:0:omit', 'on');
    const spec = parseAiSettings(settingsFixture()).protocol.apis['openai-completions']!;
    expect(readProtocolFields(form, spec)).toEqual({ compat: { supportsDeveloperRole: false, vllmPriority: 0, chatTemplateKwargs: { reasoning: { $var: 'thinking.enabled', omitWhenOff: true } } }, thinkingLevelMap: { high: 'custom-high' } });
    form.set('template:chatTemplateKwargs:1:key', 'reasoning');
    expect(() => readProtocolFields(form, spec)).toThrow('重复');
    form.set('template:chatTemplateKwargs:1:key', '__proto__');
    expect(() => readProtocolFields(form, spec)).toThrow('不合格');
  });
});
function file(relative: string, content: string | Uint8Array) {
  const bytes = typeof content === 'string' ? new TextEncoder().encode(content) : content;
  const result = new File([new Uint8Array(bytes).buffer], relative.split('/').at(-1)!);
  Object.defineProperties(result, { webkitRelativePath: { value: relative }, arrayBuffer: { value: async () => bytes.slice().buffer } });
  return result;
}
describe('explicit local skill directory selection', () => {
  const limits = parseAiSettings(settingsFixture()).resources;
  it('reads selected text and binary references without executing anything', async () => {
    const packet = await readSkillFolder([file('example/SKILL.md', '---\nname: example\n---\nSynthetic'), file('example/references/icon.bin', new Uint8Array([255, 0, 255]))], limits);
    expect(packet.directory).toBe('example'); expect(packet.files[0]?.encoding).toBe('utf8'); expect(packet.files[1]).toMatchObject({ encoding: 'base64', content: '/wD/' });
  });
  it.each([
    ['example/readme.md', 'example/references/a.txt'],
    ['example/SKILL.md', 'example/SCRIPTS/run.sh'],
    ['example/SKILL.md', 'example/references/e\u0301.txt', 'example/references/é.txt'],
    ['example/SKILL.md', 'example/references/../bad.txt'],
    ['Example/SKILL.md'],
  ])('rejects an incomplete or ambiguous package %s with a visible reason', async (...paths) => {
    await expect(readSkillFolder(paths.map(path => file(path, 'synthetic')), limits)).rejects.toThrow();
  });
  it('checks file and total byte limits before upload', async () => {
    await expect(readSkillFolder([file('example/SKILL.md', 'x'.repeat(limits.maxSkillFileBytes + 1))], limits)).rejects.toThrow('上限');
    await expect(readSkillFolder([], limits)).rejects.toThrow('SKILL.md');
  });
});
