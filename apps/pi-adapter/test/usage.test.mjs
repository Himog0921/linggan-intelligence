import test from 'node:test';
import assert from 'node:assert/strict';
import { emptyUsage, recordWireUsage } from '../src/usage.mjs';

test('completions cache and reasoning are subsets, including DeepSeek cache facts', () => {
  const usage = emptyUsage();
  recordWireUsage('openai-completions', {usage:{prompt_tokens:100,completion_tokens:40,prompt_cache_hit_tokens:60,completion_tokens_details:{reasoning_tokens:30}}}, usage);
  assert.deepEqual(usage, {inputTokens:100,outputTokens:40,cachedInputTokens:60,cachedWriteTokens:null,reasoningTokens:30,costUsd:null});
  assert.equal(usage.inputTokens + usage.outputTokens, 140);
});
test('responses retain explicit zero cache/reasoning and absent usage remains unknown', () => {
  const usage=emptyUsage();
  recordWireUsage('openai-responses',{response:{usage:{input_tokens:100,output_tokens:40,input_tokens_details:{cached_tokens:0,cache_write_tokens:0},output_tokens_details:{reasoning_tokens:0}}}},usage);
  assert.equal(usage.cachedInputTokens,0);
  assert.equal(usage.cachedWriteTokens,0);
  assert.equal(usage.reasoningTokens,0);
  recordWireUsage('openai-responses',{},usage);
  assert.equal(usage.inputTokens,100);
  const missing=emptyUsage();
  recordWireUsage('openai-completions',{usage:{prompt_tokens:100}},missing);
  assert.equal(missing.outputTokens,null);
  assert.equal(missing.cachedInputTokens,null);
  assert.equal(missing.costUsd,null);
});
test('Anthropic totals include both caches once across split usage events', () => {
  const usage=emptyUsage();
  recordWireUsage('anthropic-messages',{message:{usage:{input_tokens:50,cache_read_input_tokens:30,cache_creation_input_tokens:20}}},usage);
  recordWireUsage('anthropic-messages',{usage:{output_tokens:40}},usage);
  assert.deepEqual(usage,{inputTokens:100,outputTokens:40,cachedInputTokens:30,cachedWriteTokens:20,reasoningTokens:null,costUsd:null});
  const partial=emptyUsage();
  recordWireUsage('anthropic-messages',{usage:{input_tokens:50,output_tokens:40}},partial);
  assert.equal(partial.inputTokens,null,'uncached input is not an inclusive total');
  assert.equal(partial.outputTokens,40);
});
test('invalid or contradictory measurements are rejected before becoming ledger facts', () => {
  for (const usage of [
    {prompt_tokens:-1}, {prompt_tokens:0.5}, {prompt_tokens:Number.MAX_SAFE_INTEGER+1},
    {prompt_tokens:10,prompt_tokens_details:{cached_tokens:11}},
    {completion_tokens:10,completion_tokens_details:{reasoning_tokens:11}},
  ]) assert.throws(()=>recordWireUsage('openai-completions',{usage},emptyUsage()),/provider_invalid_usage/);
});

test('a contradictory later event cannot corrupt an earlier valid wire receipt', () => {
  const usage=emptyUsage();
  recordWireUsage('openai-completions',{usage:{prompt_tokens:20,completion_tokens:5}},usage);
  const before={...usage};
  assert.throws(()=>recordWireUsage('openai-completions',{usage:{prompt_tokens:10,prompt_tokens_details:{cached_tokens:11}}},usage));
  assert.deepEqual(usage,before);
});
