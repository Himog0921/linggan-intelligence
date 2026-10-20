import LIMITS from '../../../crates/contracts/src/ai_limits.json' with { type: 'json' };
import PROTOCOL from '../../../crates/contracts/src/ai_protocol.json' with { type: 'json' };
export { LIMITS, PROTOCOL };
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const bytes = value => Buffer.byteLength(typeof value === 'string' ? value : JSON.stringify(value));
const integer = (value, min, max) => Number.isSafeInteger(value) && value >= min && value <= max;
const safeKey = key => bytes(key) > 0 && bytes(key) <= 80 && !['__proto__', 'prototype', 'constructor'].includes(key);
function invalid() { throw new Error('invalid_request'); }
function keys(value, allowed) { return object(value) && Object.keys(value).every(k => allowed.includes(k)); }
function templateValue(value) {
  if (typeof value === 'boolean') return true;
  if (typeof value === 'number') return Number.isFinite(value);
  if (typeof value === 'string') return bytes(value) <= 200;
  return keys(value, ['$var', 'omitWhenOff']) && ['thinking.enabled', 'thinking.effort', 'thinking.budget'].includes(value.$var)
    && (value.omitWhenOff === undefined || typeof value.omitWhenOff === 'boolean');
}
export function validCompatibility(api, value) {
  const spec = PROTOCOL.apis[api];
  if (!spec || !object(value) || bytes(value) > 16384) return false;
  return Object.entries(value).every(([key, value]) => {
    if (!safeKey(key)) return false;
    if (spec.booleans.includes(key)) return typeof value === 'boolean';
    if (typeof value === 'string') return spec.enums[key]?.includes(value) === true;
    if (spec.templates.includes(key)) return object(value) && Object.entries(value).every(([k, v]) => safeKey(k) && templateValue(v));
    const bounds = spec.numbers[key];
    return bounds !== undefined && integer(value, bounds.min, bounds.max);
  });
}
export function validOutputMode(api, mode, strict) {
  return PROTOCOL.apis[api]?.outputModes.includes(mode) === true && (!strict || mode === 'json_schema')
    && !(api === 'anthropic-messages' && mode === 'json_schema' && !strict);
}
export function executionPolicy(r) {
  const spec = PROTOCOL.apis[r.api];
  if (!spec) invalid();
  if (r.execution === undefined || r.execution === null) {
    if (!integer(r.timeoutMs, 100, LIMITS.legacyTimeoutSeconds * 1000)
      || !integer(r.maxOutputTokens, 16, LIMITS.legacyOutputTokens)) invalid();
    return { qualified: false, modelReasoning: false, thinkingAllowed: false,
      compat: { ...spec.defaults }, outputMode: 'prompt_only', strictSchema: false, thinkingLevelMap: {} };
  }
  const e = r.execution;
  if (!keys(e, ['capabilityRef', 'limits', 'inputTokenLimit', 'thinkingLevel', 'compat', 'thinkingLevelMap', 'outputMode', 'strictSchema', 'responseSchema'])
    || typeof e.capabilityRef !== 'string' || !/^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/i.test(e.capabilityRef)
    || !keys(e.limits, ['contextTokens', 'maxOutputTokens', 'maxTimeoutSeconds', 'thinkingLevels'])) invalid();
  const c = e.limits;
  if (!integer(c.contextTokens, 1024, LIMITS.maxContextTokens)
    || !integer(c.maxOutputTokens, 1, Math.min(c.contextTokens, LIMITS.maxOutputTokens))
    || !integer(c.maxTimeoutSeconds, 1, LIMITS.maxTimeoutSeconds)
    || !Array.isArray(c.thinkingLevels) || c.thinkingLevels.length === 0
    || new Set(c.thinkingLevels).size !== c.thinkingLevels.length
    || !c.thinkingLevels.every(level => PROTOCOL.thinkingLevels.includes(level))
    || !c.thinkingLevels.includes(e.thinkingLevel)
    || !integer(e.inputTokenLimit, 1024, LIMITS.maxContextTokens)
    || e.inputTokenLimit + r.maxOutputTokens > c.contextTokens
    || r.maxOutputTokens > c.maxOutputTokens || r.timeoutMs > c.maxTimeoutSeconds * 1000
    || !validCompatibility(r.api, e.compat) || !object(e.thinkingLevelMap) || bytes(e.thinkingLevelMap) > 4096
    || typeof e.strictSchema !== 'boolean' || !validOutputMode(r.api, e.outputMode, e.strictSchema)) invalid();
  for (const [level, value] of Object.entries(e.thinkingLevelMap)) {
    if (!PROTOCOL.thinkingLevels.includes(level) || (value !== null && (typeof value !== 'string' || !value.trim() || bytes(value) > 80))
      || (value === null && c.thinkingLevels.includes(level))) invalid();
  }
  const compat = { ...spec.defaults, ...e.compat };
  if (compat.supportsMidConvoEffort && e.thinkingLevel === 'off') invalid();
  if (r.api === 'openai-responses' && compat.supportsMaxOutputTokens === false) invalid();
  if (r.api === 'anthropic-messages' && e.thinkingLevel !== 'off' && !compat.forceAdaptiveThinking && !compat.supportsMidConvoEffort
    && r.maxOutputTokens < spec.minBudgetThinkingTokens + spec.minThinkingAnswerTokens) invalid();
  if (e.thinkingLevel !== 'off' && r.api === 'openai-completions' && compat.thinkingFormat === 'openai' && !compat.supportsReasoningEffort) invalid();
  const thinkingLevelMap = Object.fromEntries(PROTOCOL.thinkingLevels.map(level => [level,
    !c.thinkingLevels.includes(level) ? null : (e.thinkingLevelMap[level] ?? (level === 'off' ? 'none' : level))]));
  const modelReasoning = c.thinkingLevels.some(level => level !== 'off') || typeof e.thinkingLevelMap.off === 'string'
    || (e.compat.thinkingFormat !== undefined && e.compat.thinkingFormat !== 'openai');
  return { qualified: true, contextWindow: c.contextTokens, inputTokenLimit: e.inputTokenLimit,
    maxTokens: c.maxOutputTokens, reasoning: e.thinkingLevel === 'off' ? undefined : e.thinkingLevel,
    thinkingAllowed: e.thinkingLevel !== 'off', modelReasoning, thinkingLevelMap, compat,
    outputMode: e.outputMode, strictSchema: e.strictSchema, responseSchema: e.responseSchema };
}

// This checks only schema transport shape. Rust remains the business-result authority.
export function completeSchema(schema) {
  let nodes = 0;
  const visit = (s, depth = 0) => {
    if (!object(s) || depth > 20 || ++nodes > 256 || s.$ref !== undefined) return false;
    if (Array.isArray(s.anyOf)) return s.anyOf.length > 0 && s.anyOf.every(v => visit(v, depth + 1));
    const types = Array.isArray(s.type) ? s.type : [s.type];
    if (!types.length || !types.every(t => ['object', 'array', 'string', 'integer', 'number', 'boolean', 'null'].includes(t))) return false;
    if (types.includes('object')) {
      if (!object(s.properties) || s.additionalProperties !== false || !Array.isArray(s.required)) return false;
      const propertyKeys = Object.keys(s.properties);
      if (new Set(s.required).size !== propertyKeys.length || s.required.length !== propertyKeys.length
        || !propertyKeys.every(k => s.required.includes(k) && visit(s.properties[k], depth + 1))) return false;
    }
    return !types.includes('array') || visit(s.items, depth + 1);
  };
  return visit(schema);
}
export function structuredPayload(r, policy, packetSchema) {
  if (policy.outputMode === 'prompt_only') return null;
  if (policy.outputMode === 'json_object') {
    if (r.api === 'openai-completions') return { response_format: { type: 'json_object' } };
    if (r.api === 'openai-responses') return { text: { format: { type: 'json_object' } } };
    invalid();
  }
  const response = policy.responseSchema ?? packetSchema;
  if (!keys(response, ['name', 'schema']) || !/^[a-zA-Z0-9_-]{1,64}$/.test(response.name)
    || !object(response.schema) || response.schema.type !== 'object' || !object(response.schema.properties)
    || response.schema.additionalProperties !== false || bytes(response.schema) > LIMITS.maxResponseSchemaBytes
    || (policy.strictSchema && !completeSchema(response.schema))) invalid();
  const { name, schema } = response;
  const strict = policy.strictSchema ? { strict: true } : {};
  if (r.api === 'openai-completions') return { response_format: { type: 'json_schema', json_schema: { name, schema, ...strict } } };
  if (r.api === 'openai-responses') return { text: { format: { type: 'json_schema', name, schema, ...strict } } };
  if (r.api === 'anthropic-messages') return { output_config: { format: { type: 'json_schema', schema } } };
  invalid();
}

// Some SDK helpers add a thinking allowance or subtract a context heuristic. The
// final payload must still carry exactly the caller's frozen output ceiling.
export function boundedPayload(r, policy, payload, outputFormat) {
  const formatted = { ...payload, ...outputFormat };
  if (outputFormat?.output_config) formatted.output_config = { ...payload.output_config, ...outputFormat.output_config };
  if (formatted.tools?.length || formatted.additional_tools?.length) invalid();
  if (r.api === 'openai-completions') {
    const field = policy.compat.maxTokensField;
    delete formatted.max_tokens;
    delete formatted.max_completion_tokens;
    formatted[field] = r.maxOutputTokens;
  } else if (r.api === 'openai-responses') {
    if (policy.compat.supportsMaxOutputTokens === false) invalid();
    formatted.max_output_tokens = r.maxOutputTokens;
  } else if (r.api === 'anthropic-messages') {
    formatted.max_tokens = r.maxOutputTokens;
    if (formatted.thinking?.type === 'enabled') {
      const spec = PROTOCOL.apis[r.api];
      if (!policy.thinkingAllowed || r.maxOutputTokens < spec.minBudgetThinkingTokens + spec.minThinkingAnswerTokens) invalid();
      formatted.thinking = { ...formatted.thinking, budget_tokens: Math.max(spec.minBudgetThinkingTokens,
        Math.min(formatted.thinking.budget_tokens ?? spec.minBudgetThinkingTokens, r.maxOutputTokens - spec.minThinkingAnswerTokens)) };
    }
  }
  return formatted;
}
