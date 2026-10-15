import { ApiError, isRecord, requestJson } from '../../shared/api';

/** 读写合同：`apps/api/src/local_web/model_settings.rs` 与 `crates/intelligence/src/model_settings*.rs`。 */
const API = '/api/local/model-settings';

export interface ConnectionTest {
  failureCode: string | null;
}

export interface ModelConnection {
  connectionRef: string;
  versionRef: string;
  name: string;
  api: string;
  baseUrl: string;
  localEndpoint: boolean;
  enabled: boolean;
  revision: number;
  test: ConnectionTest | null;
}

export interface ModelEntry {
  modelRef: string;
  modelId: string;
  connectionVersionRef: string;
  connectionName: string;
  semanticQualified: boolean;
  modelCallable: boolean;
  enabled: boolean;
}

export interface ModelConfig {
  configRef: string;
  modelRef: string;
  inputTokenLimit: number;
  outputTokenLimit: number;
  timeoutSeconds: number;
  maxAttempts: number;
}

export interface WorkerState {
  lastSeenAt: string | null;
  recent: boolean;
  lastError: string | null;
}

export interface Invocation {
  invocationRef: string;
  operation: string;
  state: string;
  failureCode: string | null;
  inputTokens: number | null;
  outputTokens: number | null;
  chargedTokens: number;
  createdAt: string;
  modelId: string | null;
}

export interface ModelSettings {
  worker: WorkerState;
  connections: ModelConnection[];
  models: ModelEntry[];
  config: ModelConfig | null;
  invocations: Invocation[];
}

class ShapeError extends Error {}

function field(record: Record<string, unknown>, name: string): unknown {
  if (!(name in record)) throw new ShapeError(name);
  return record[name];
}
function str(record: Record<string, unknown>, name: string): string {
  const value = field(record, name);
  if (typeof value !== 'string') throw new ShapeError(name);
  return value;
}
function optStr(record: Record<string, unknown>, name: string): string | null {
  const value = record[name];
  if (value === undefined || value === null) return null;
  if (typeof value !== 'string') throw new ShapeError(name);
  return value;
}
function bool(record: Record<string, unknown>, name: string): boolean {
  const value = field(record, name);
  if (typeof value !== 'boolean') throw new ShapeError(name);
  return value;
}
function num(record: Record<string, unknown>, name: string): number {
  const value = field(record, name);
  if (typeof value !== 'number' || !Number.isFinite(value)) throw new ShapeError(name);
  return value;
}
function optNum(record: Record<string, unknown>, name: string): number | null {
  const value = record[name];
  if (value === undefined || value === null) return null;
  if (typeof value !== 'number' || !Number.isFinite(value)) throw new ShapeError(name);
  return value;
}
function list<T>(record: Record<string, unknown>, name: string, parse: (item: Record<string, unknown>) => T): T[] {
  const value = field(record, name);
  if (!Array.isArray(value)) throw new ShapeError(name);
  return value.map(item => {
    if (!isRecord(item)) throw new ShapeError(name);
    return parse(item);
  });
}

function parseConnection(row: Record<string, unknown>): ModelConnection {
  const test = row.test;
  return {
    connectionRef: str(row, 'connectionRef'),
    versionRef: str(row, 'versionRef'),
    name: str(row, 'name'),
    api: str(row, 'api'),
    baseUrl: str(row, 'baseUrl'),
    localEndpoint: bool(row, 'localEndpoint'),
    enabled: bool(row, 'enabled'),
    revision: num(row, 'revision'),
    test: isRecord(test) ? { failureCode: optStr(test, 'failureCode') } : null,
  };
}

function parseModel(row: Record<string, unknown>): ModelEntry {
  return {
    modelRef: str(row, 'modelRef'),
    modelId: str(row, 'modelId'),
    connectionVersionRef: str(row, 'connectionVersionRef'),
    connectionName: str(row, 'connectionName'),
    semanticQualified: bool(row, 'semanticQualified'),
    modelCallable: bool(row, 'modelCallable'),
    enabled: bool(row, 'enabled'),
  };
}

function parseConfig(value: unknown): ModelConfig | null {
  if (value === null || value === undefined) return null;
  if (!isRecord(value)) throw new ShapeError('config');
  return {
    configRef: str(value, 'configRef'),
    modelRef: str(value, 'modelRef'),
    inputTokenLimit: num(value, 'inputTokenLimit'),
    outputTokenLimit: num(value, 'outputTokenLimit'),
    timeoutSeconds: num(value, 'timeoutSeconds'),
    maxAttempts: num(value, 'maxAttempts'),
  };
}

function parseInvocation(row: Record<string, unknown>): Invocation {
  return {
    invocationRef: str(row, 'invocationRef'),
    operation: str(row, 'operation'),
    state: str(row, 'state'),
    failureCode: optStr(row, 'failureCode'),
    inputTokens: optNum(row, 'inputTokens'),
    outputTokens: optNum(row, 'outputTokens'),
    chargedTokens: num(row, 'chargedTokens'),
    createdAt: str(row, 'createdAt'),
    modelId: optStr(row, 'modelId'),
  };
}

/** 运行时校验读取结果。字段缺失或类型不对时报「返回内容无法解析」，不拿残缺数据渲染。 */
export function parseModelSettings(value: unknown): ModelSettings {
  try {
    if (!isRecord(value)) throw new ShapeError('root');
    const worker = field(value, 'worker');
    if (!isRecord(worker)) throw new ShapeError('worker');
    return {
      worker: {
        lastSeenAt: optStr(worker, 'lastSeenAt'),
        recent: worker.recent === true,
        lastError: optStr(worker, 'lastError'),
      },
      connections: list(value, 'connections', parseConnection),
      models: list(value, 'models', parseModel),
      config: parseConfig(value.config),
      invocations: list(value, 'invocations', parseInvocation),
    };
  } catch (error) {
    if (error instanceof ShapeError) throw new ApiError({ kind: 'invalid_response', status: 200 });
    throw error;
  }
}

export async function fetchModelSettings(signal?: AbortSignal): Promise<ModelSettings> {
  return parseModelSettings(await requestJson(API, signal ? { signal } : {}));
}

export interface SaveConnectionRequest {
  versionRef: string;
  connectionRef: string;
  expectedRevision: number;
  name: string;
  api: string;
  baseUrl: string;
  apiKey: string;
  localEndpoint: boolean;
}

/** 首次保存与重放（同一 versionRef 再次提交）都会带回连接的当前修订号。 */
export async function saveConnection(request: SaveConnectionRequest): Promise<{ versionRef: string; revision: number }> {
  const body = await requestJson(`${API}/connections`, { method: 'POST', body: request });
  if (!isRecord(body) || typeof body.versionRef !== 'string' || typeof body.revision !== 'number') {
    throw new ApiError({ kind: 'invalid_response', status: 200 });
  }
  return { versionRef: body.versionRef, revision: body.revision };
}

export interface SaveModelRequest {
  modelRef: string;
  connectionVersionRef: string;
  modelId: string;
}

export async function saveModel(request: SaveModelRequest): Promise<{ modelRef: string }> {
  const body = await requestJson(`${API}/models`, { method: 'POST', body: request });
  if (!isRecord(body) || typeof body.modelRef !== 'string') {
    throw new ApiError({ kind: 'invalid_response', status: 200 });
  }
  return { modelRef: body.modelRef };
}

export interface SetConnectionEnabledRequest {
  connectionRef: string;
  expectedRevision: number;
  enabled: boolean;
}

export async function setConnectionEnabled(request: SetConnectionEnabledRequest): Promise<void> {
  await requestJson(`${API}/connections/state`, { method: 'POST', body: request });
}

export interface ProbeRequest {
  invocationRef: string;
  connectionVersionRef: string;
  modelRef: string;
  operation: 'probe';
}

/** 测试的结论。重放（同一请求编号再次提交）时服务器把结果放在 `result` 里。 */
export type ProbeOutcome =
  | { kind: 'qualified' }
  | { kind: 'callable_not_qualified' }
  | { kind: 'failed'; failureCode: string | null }
  | { kind: 'still_running' };

export function readProbeOutcome(body: unknown): ProbeOutcome {
  if (!isRecord(body)) throw new ApiError({ kind: 'invalid_response', status: 200 });
  let result: unknown = body;
  if (body.replayed === true) {
    if (body.result === null || body.result === undefined) return { kind: 'still_running' };
    result = body.result;
  }
  if (!isRecord(result)) throw new ApiError({ kind: 'invalid_response', status: 200 });
  if (result.semanticQualified === true) return { kind: 'qualified' };
  if (result.modelCallable === true) return { kind: 'callable_not_qualified' };
  return { kind: 'failed', failureCode: typeof result.failureCode === 'string' ? result.failureCode : null };
}

export async function probeModel(request: ProbeRequest): Promise<ProbeOutcome> {
  return readProbeOutcome(await requestJson(`${API}/probes`, { method: 'POST', body: request }));
}

export interface SaveConfigRequest {
  configRef: string;
  expectedConfigRef: string | null;
  modelRef: string;
  inputTokenLimit: number;
  outputTokenLimit: number;
  timeoutSeconds: number;
  maxAttempts: number;
}

export async function saveConfig(request: SaveConfigRequest): Promise<void> {
  await requestJson(`${API}/config`, { method: 'POST', body: request });
}
