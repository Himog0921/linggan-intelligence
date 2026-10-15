/**
 * 同源 JSON 请求。只负责传输与回执的基本校验：不重新判断业务结论，不把失败变成空数据，
 * 也不自动重试写操作——写操作的重试必须由调用方带着原请求身份显式发起。
 */
export type ApiFailure =
  /** 服务器给出了明确答复（非 2xx）。`code` 是合同里的错误码，读不到时为 null。 */
  | { kind: 'http'; status: number; code: string | null }
  /** 请求没有得到答复：结果未确认，写操作可能已经生效。 */
  | { kind: 'network' }
  /** 得到了答复，但内容不是约定的 JSON。 */
  | { kind: 'invalid_response'; status: number };

export class ApiError extends Error {
  readonly failure: ApiFailure;
  /** 仅保留本次内存中的 JSON 回答，由业务 DTO 解码；不记录或持久化来源原文。 */
  readonly responseBody: unknown;

  constructor(failure: ApiFailure, responseBody?: unknown) {
    super(failureSummary(failure));
    this.name = 'ApiError';
    this.failure = failure;
    this.responseBody = responseBody;
  }

  /** 服务器是否给出了明确答复。只有明确答复之后，写操作才可以换新的请求身份。 */
  get answered(): boolean {
    return this.failure.kind === 'http';
  }
}

function failureSummary(failure: ApiFailure): string {
  switch (failure.kind) {
    case 'http':
      return `HTTP ${failure.status}${failure.code ? ` ${failure.code}` : ''}`;
    case 'network':
      return '请求没有得到答复';
    case 'invalid_response':
      return `HTTP ${failure.status} 返回内容无法解析`;
  }
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

interface RequestOptions {
  method?: 'GET' | 'POST';
  body?: unknown;
  signal?: AbortSignal;
}

export async function requestJson(path: string, options: RequestOptions = {}): Promise<unknown> {
  const headers: Record<string, string> = { Accept: 'application/json' };
  if (options.body !== undefined) headers['Content-Type'] = 'application/json';
  return sendJsonRequest(path, options.method ?? 'GET', headers,
    options.body === undefined ? null : JSON.stringify(options.body), options.signal);
}

interface BinaryRequestOptions {
  method: 'POST' | 'PUT';
  contentType: string;
  requestId?: string;
  signal?: AbortSignal;
}

/** 图片上传使用同一传输边界；幂等身份由调用方持有，传输层不生成、不重试。 */
export async function requestBinaryJson(path: string, body: Blob, options: BinaryRequestOptions): Promise<unknown> {
  const headers: Record<string, string> = {
    Accept: 'application/json', 'Content-Type': options.contentType,
  };
  if (options.requestId !== undefined) headers['x-creation-request-id'] = options.requestId;
  return sendJsonRequest(path, options.method, headers, body, options.signal);
}

async function sendJsonRequest(
  path: string, method: string, headers: Record<string, string>, body: BodyInit | null, signal?: AbortSignal,
): Promise<unknown> {
  if (!path.startsWith('/') || path.startsWith('//') || /[\\\r\n\t]/.test(path)) {
    throw new Error('只允许同源相对路径');
  }
  let text: string;
  let response: Response;
  try {
    response = await fetch(path, {
      method,
      cache: 'no-store',
      credentials: 'same-origin',
      headers,
      body,
      signal: signal ?? null,
    });
    text = await response.text();
  } catch (error) {
    // 主动取消不是失败，原样抛出交给调用方忽略。
    if (signal?.aborted) throw error;
    throw new ApiError({ kind: 'network' });
  }
  let parsed: unknown;
  try {
    parsed = text === '' ? null : JSON.parse(text);
  } catch {
    parsed = undefined;
  }
  if (!response.ok) {
    const code = isRecord(parsed)
      ? typeof parsed.error === 'string' ? parsed.error
        : isRecord(parsed.error) && typeof parsed.error.code === 'string' ? parsed.error.code
          : typeof parsed.code === 'string' ? parsed.code : null
      : null;
    throw new ApiError({ kind: 'http', status: response.status, code }, parsed);
  }
  const contentType = response.headers.get('Content-Type')?.split(';')[0]?.trim().toLowerCase();
  if (parsed === undefined || !(contentType === 'application/json' || contentType?.endsWith('+json'))) {
    throw new ApiError({ kind: 'invalid_response', status: response.status });
  }
  return parsed;
}
