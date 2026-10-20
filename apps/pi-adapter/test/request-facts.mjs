// Constructed capability facts. They prove the pinned transport, not a real vendor's capacity.
export function capabilityFacts(api, overrides = {}) {
  return {
    capabilityRef: '00000000-0000-4000-8000-000000000001',
    limits: { contextTokens: 1_000_000, maxOutputTokens: 384_000, maxTimeoutSeconds: 3600, thinkingLevels: ['off', 'high', 'max'] },
    inputTokenLimit: 32768, thinkingLevel: 'off',
    compat: api === 'openai-completions' ? { thinkingFormat: 'deepseek', supportsReasoningEffort: true } : {},
    thinkingLevelMap: { off: 'none', high: 'high', max: 'max' },
    outputMode: api === 'openai-completions' ? 'json_object' : 'json_schema', strictSchema: false,
    ...overrides,
  };
}
