import { pinnedSdkAvailable } from './sdk-version.mjs';
// Only files under the pinned package's own static data directory are read. There
// is no provider refresh, authentication lookup, user file path, or network call.
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { getSupportedThinkingLevels } from '@earendil-works/pi-ai';
import { LIMITS, PROTOCOL, validCompatibility } from './execution.mjs';
const directory = new URL('./providers/data/', import.meta.resolve('@earendil-works/pi-ai'));
const apis = new Set(['openai-completions', 'openai-responses', 'anthropic-messages']);
function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (value !== null && typeof value === 'object') return `{${Object.keys(value).sort().map(k => `${JSON.stringify(k)}:${canonical(value[k])}`).join(',')}}`;
  return JSON.stringify(value);
}
export function catalogRate(value) {
  // Catalog zeroes can be SDK placeholders. A known zero price requires explicit manual input.
  if (typeof value !== 'number' || !Number.isFinite(value) || value <= 0) return null;
  const match = /^(\d+)(?:\.(\d+))?$/.exec(String(value));
  if (!match) return null;
  const fraction = (match[2] ?? '').replace(/0+$/, '');
  if (fraction.length > 6) return null;
  const rate = BigInt(match[1]) * 1_000_000n + BigInt(fraction.padEnd(6, '0'));
  return rate <= 9223372036854775807n ? String(rate) : null;
}
function preset(m) {
  const sourceVersion = `pi-${PROTOCOL.sdkVersion}:${createHash('sha256').update(canonical({contract:PROTOCOL.contract, model:m})).digest('hex')}`;
  const reasons = [];
  if (!Number.isSafeInteger(m.contextWindow) || m.contextWindow < 1024 || m.contextWindow > LIMITS.maxContextTokens
    || !Number.isSafeInteger(m.maxTokens) || m.maxTokens < 1 || m.maxTokens > Math.min(m.contextWindow, LIMITS.maxOutputTokens)) reasons.push('resource_metadata_unsupported');
  if (!m.input?.includes('text')) reasons.push('text_input_unavailable');
  if (m.headers && Object.keys(m.headers).length) reasons.push('model_headers_require_manual_configuration');
  if (!validCompatibility(m.api, m.compat ?? {})) reasons.push('compatibility_metadata_unsupported');
  if (m.api==='openai-responses'&&m.compat?.supportsMaxOutputTokens===false) reasons.push('output_ceiling_unavailable');
  const thinkingLevels=getSupportedThinkingLevels(m).filter(level=>!(level==='off'&&m.compat?.supportsMidConvoEffort===true));
  if(thinkingLevels.length===0)reasons.push('thinking_metadata_inconsistent');
  return { provider:m.provider, modelId:m.id, name:m.name ?? m.id, api:m.api, sourceVersion, supported:reasons.length === 0, reasons,
    limits:{contextTokens:m.contextWindow,maxOutputTokens:m.maxTokens,maxTimeoutSeconds:null,thinkingLevels},
    compat:m.compat ?? {},thinkingLevelMap:m.thinkingLevelMap ?? {},outputMode:'prompt_only',strictSchema:false,
    prices:{input:catalogRate(m.cost?.input),output:catalogRate(m.cost?.output),cachedRead:catalogRate(m.cost?.cacheRead),cachedWrite:catalogRate(m.cost?.cacheWrite)},
    operatorFields:['maxTimeoutSeconds'],unknownFields:['nativeStructuredOutput','currentEndpointCallability','currentPrice'],
  };
}
export function readCatalog(request = {}) {
  if(!pinnedSdkAvailable())throw new Error('catalog_unavailable');
  const allowed = ['api','provider','query','modelId','sourceVersion','offset','limit'];
  if (!request || typeof request !== 'object' || Array.isArray(request) || Object.keys(request).some(k => !allowed.includes(k))) throw new Error('invalid_request');
  for (const field of ['api','provider','query','modelId','sourceVersion']) if (request[field] !== undefined && request[field] !== null
    && (typeof request[field] !== 'string' || Buffer.byteLength(request[field]) > 200)) throw new Error('invalid_request');
  if (request.api && !apis.has(request.api)) throw new Error('invalid_request');
  const { offset=0, limit=50 } = request;
  if (!Number.isInteger(offset) || offset < 0 || offset > 10000 || !Number.isInteger(limit) || limit < 1 || limit > 100) throw new Error('invalid_request');
  const all = [];
  for (const file of fs.readdirSync(directory).filter(name => name.endsWith('.json')).sort()) {
    const data = JSON.parse(fs.readFileSync(new URL(file, directory), 'utf8'));
    for (const [api, models] of Object.entries(data)) if (apis.has(api)) for (const model of Object.values(models)) all.push(model);
  }
  const providers=[...new Set(all.map(m=>m.provider))].sort();
  const query = request.query?.toLowerCase() ?? '';
  const selected = all.filter(m => (!request.api || m.api === request.api) && (!request.provider || m.provider === request.provider)
    && (!request.modelId || m.id === request.modelId) && (!query || `${m.provider} ${m.id} ${m.name}`.toLowerCase().includes(query)))
    .map(preset).filter(m => !request.sourceVersion || m.sourceVersion === request.sourceVersion)
    .sort((a,b)=>{const left=`${a.provider}\0${a.api}\0${a.modelId}`,right=`${b.provider}\0${b.api}\0${b.modelId}`;return left<right?-1:left>right?1:0;});
  return {contract:'ai-pi-catalog.v1',sdkVersion:PROTOCOL.sdkVersion,providers,total:selected.length,offset,limit,
    presets:selected.slice(offset,offset+limit),hasMore:offset+limit<selected.length};
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const chunks=[];let size=0;
    for await (const chunk of process.stdin) {size+=chunk.length;if(size>4096)throw new Error('invalid_request');chunks.push(chunk);}
    process.stdout.write(JSON.stringify(readCatalog(JSON.parse(Buffer.concat(chunks).toString('utf8'))))+'\n');
  } catch {process.stdout.write(JSON.stringify({contract:'ai-pi-catalog-error.v1',error:'catalog_unavailable'})+'\n');}
}
