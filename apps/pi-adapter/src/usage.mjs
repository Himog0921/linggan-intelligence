// Only wire observations enter the ledger. The SDK's default zero counters and
// default prices are not measurements. Cache and reasoning are subsets of totals.
const anthropicFacts = new WeakMap();
const has = (o, k) => Object.hasOwn(o, k) && o[k] !== null && o[k] !== undefined;
function token(value) {
  if (!Number.isSafeInteger(value) || value < 0 || value > 100_000_000) throw new RangeError('provider_invalid_usage');
  return value;
}
export function emptyUsage() {
  return { inputTokens: null, outputTokens: null, cachedInputTokens: null, cachedWriteTokens: null, reasoningTokens: null, costUsd: null };
}
function first(...values) { return values.find(v => v !== null && v !== undefined); }
function assign(usage, key, value) { if (value !== undefined && value !== null) usage[key] = token(value); }
export function recordWireUsage(api, value, usage) {
  const u = value.usage ?? value.message?.usage ?? value.response?.usage;
  if (!u || typeof u !== 'object' || Array.isArray(u)) return;
  const next = { ...usage };
  const facts = { ...(anthropicFacts.get(usage) ?? {}) };
  if (api === 'anthropic-messages') {
    for (const key of ['input_tokens', 'cache_read_input_tokens', 'cache_creation_input_tokens']) {
      if (has(u, key)) facts[key] = token(u[key]);
    }
    assign(next, 'cachedInputTokens', facts.cache_read_input_tokens);
    assign(next, 'cachedWriteTokens', facts.cache_creation_input_tokens);
    // Anthropic's input_tokens excludes caches. Until both subsets are reported,
    // the inclusive input total stays unknown rather than silently adding zeros.
    if (Object.keys(facts).length === 3) {
      next.inputTokens = token(facts.input_tokens + facts.cache_read_input_tokens + facts.cache_creation_input_tokens);
    }
    assign(next, 'outputTokens', u.output_tokens);
  } else {
    assign(next, 'inputTokens', first(u.prompt_tokens, u.input_tokens));
    assign(next, 'outputTokens', first(u.completion_tokens, u.output_tokens));
    assign(next, 'cachedInputTokens', first(u.prompt_tokens_details?.cached_tokens, u.input_tokens_details?.cached_tokens, u.prompt_cache_hit_tokens, u.cached_tokens));
    assign(next, 'cachedWriteTokens', first(u.prompt_tokens_details?.cache_write_tokens, u.input_tokens_details?.cache_write_tokens));
    assign(next, 'reasoningTokens', first(u.completion_tokens_details?.reasoning_tokens, u.output_tokens_details?.reasoning_tokens));
  }
  const cache = (next.cachedInputTokens ?? 0) + (next.cachedWriteTokens ?? 0);
  if ((next.inputTokens !== null && cache > next.inputTokens)
    || (next.outputTokens !== null && next.reasoningTokens !== null && next.reasoningTokens > next.outputTokens)) {
    throw new RangeError('provider_invalid_usage');
  }
  Object.assign(usage, next);
  if (api === 'anthropic-messages') anthropicFacts.set(usage, facts);
}
