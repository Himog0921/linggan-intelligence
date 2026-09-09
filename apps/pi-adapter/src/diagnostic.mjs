const RETRY_AFTER_COOLDOWN = new Set(['provider_rate_limited', 'provider_unavailable']);
const RETRY_MANUAL_REVIEW = new Set([
  'provider_timeout',
  'provider_network_error',
  'provider_stream_interrupted',
  'provider_terminal_missing',
  'provider_failed',
]);

export function createObservation() {
  return {
    bodyCompleted: false,
    requestSent: false,
    diagnostic: {
      schemaVersion: 1,
      stage: 'request_not_started',
      httpStatus: null,
      responseStarted: null,
      terminalReceived: null,
      receivedBytes: null,
      finishReason: null,
      elapsedMs: 0,
      usageKnown: false,
      sdkErrorType: null,
      retryClass: 'unknown',
    },
  };
}

export function requestSent(observation) {
  observation.requestSent = true;
  observation.diagnostic.stage = 'request_sent';
}

export function responseStarted(observation, httpStatus) {
  const diagnostic = observation.diagnostic;
  diagnostic.stage = 'response_started';
  diagnostic.httpStatus = httpStatus;
  diagnostic.responseStarted = true;
}

export function receivedChunk(observation, byteLength) {
  const diagnostic = observation.diagnostic;
  diagnostic.stage = 'streaming';
  diagnostic.receivedBytes = (diagnostic.receivedBytes ?? 0) + byteLength;
}

export function bodyCompleted(observation) {
  observation.bodyCompleted = true;
  if (observation.diagnostic.receivedBytes === null) observation.diagnostic.receivedBytes = 0;
}

function normalizeFinishReason(value) {
  switch (value) {
    case 'stop':
    case 'end':
    case 'end_turn':
    case 'pause_turn':
    case 'stop_sequence':
      return 'stop';
    case 'length':
    case 'max_tokens':
      return 'length';
    case 'content_filter':
    case 'sensitive':
      return 'content_filter';
    case 'tool_calls':
    case 'tool_call':
    case 'function_call':
    case 'tool_use':
    case 'toolUse':
      return 'tool_calls';
    default:
      return value == null ? null : 'unknown';
  }
}

export function terminalReceived(observation, reason) {
  const diagnostic = observation.diagnostic;
  diagnostic.stage = 'terminal';
  diagnostic.terminalReceived = true;
  const normalized = normalizeFinishReason(reason);
  if (normalized !== null && (diagnostic.finishReason === null || diagnostic.finishReason === 'unknown')) {
    diagnostic.finishReason = normalized;
  }
}

export function succeeded(observation, reason = null) {
  terminalReceived(observation, reason);
  observation.diagnostic.sdkErrorType = null;
  observation.diagnostic.retryClass = 'never';
}

export function rejected(observation) {
  const diagnostic = observation.diagnostic;
  diagnostic.stage = 'rejected';
  diagnostic.retryClass = 'never';
}

function sdkErrorFor(code) {
  switch (code) {
    case 'provider_timeout':
      return 'timeout';
    case 'provider_network_error':
      return 'network';
    case 'provider_stream_interrupted':
      return 'stream_interrupted';
    case 'provider_terminal_missing':
    case 'provider_content_filtered':
    case 'response_too_large':
      return 'invalid_response';
    case 'provider_rate_limited':
    case 'provider_unavailable':
    case 'authentication_failed':
    case 'provider_request_rejected':
    case 'provider_endpoint_not_found':
    case 'catalog_unavailable':
    case 'provider_redirect_rejected':
      return 'http';
    default:
      return code === 'provider_failed' ? 'sdk' : null;
  }
}

export function failed(observation, code) {
  const diagnostic = observation.diagnostic;
  if (code === 'provider_timeout') {
    diagnostic.stage = 'timed_out';
  } else if (diagnostic.terminalReceived !== true && diagnostic.stage !== 'rejected') {
    diagnostic.stage = 'failed';
  }
  diagnostic.sdkErrorType = sdkErrorFor(code);
  diagnostic.retryClass = RETRY_AFTER_COOLDOWN.has(code)
    ? 'after_cooldown'
    : RETRY_MANUAL_REVIEW.has(code)
      ? 'manual_review'
      : 'never';
}

export function classifyUnterminated(observation) {
  if (observation.diagnostic.terminalReceived === true) return 'provider_failed';
  if (observation.bodyCompleted) return 'provider_terminal_missing';
  if (observation.diagnostic.responseStarted === true) return 'provider_stream_interrupted';
  if (observation.requestSent) return 'provider_network_error';
  return 'provider_failed';
}

export function completeDiagnostic(observation, usage, startedAt) {
  const diagnostic = observation.diagnostic;
  diagnostic.elapsedMs = Math.max(0, Date.now() - startedAt);
  diagnostic.usageKnown = usage.inputTokens !== null && usage.outputTokens !== null;
  return diagnostic;
}

export function finishReasonForStopReason(stopReason) {
  return normalizeFinishReason(stopReason);
}

export function observeSseLine(observation, line, usage, recordUsage) {
  if (line === 'event: message_stop') {
    terminalReceived(observation, null);
    return;
  }
  if (!line.startsWith('data:')) return;
  const data = line.slice(5).trimStart();
  if (data === '[DONE]') {
    terminalReceived(observation, null);
    return;
  }
  let value;
  try {
    value = JSON.parse(data);
  } catch {
    return;
  }
  recordUsage(value, usage);
  const choice = Array.isArray(value?.choices)
    ? value.choices.find((candidate) => candidate?.finish_reason != null)
    : null;
  if (choice) terminalReceived(observation, choice.finish_reason);
  if (value?.type === 'message_stop') terminalReceived(observation, null);
  if (value?.type === 'response.completed') terminalReceived(observation, 'stop');
  if (value?.type === 'response.incomplete') terminalReceived(observation, 'length');
  if (value?.type === 'response.failed' || value?.type === 'response.cancelled') terminalReceived(observation, 'unknown');
}
