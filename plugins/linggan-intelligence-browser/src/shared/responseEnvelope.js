function isPlainObject(value) {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

export function stripEnvelopeKeys(result = {}, extraKeys = []) {
  const data = { ...result };
  delete data.success;
  delete data.error;
  delete data.data;
  for (const key of extraKeys) {
    delete data[key];
  }
  return data;
}

export function normalizeCompatResponse(result, options = {}) {
  const {
    dataValue,
    extraStripKeys = [],
    successWhenUndefined = true,
  } = options;

  if (isPlainObject(result)) {
    const success = typeof result.success === 'boolean'
      ? result.success
      : !String(result.error || '').trim();
    return {
      success,
      ...result,
      data: 'data' in result ? result.data : (typeof dataValue === 'undefined'
        ? stripEnvelopeKeys(result, extraStripKeys)
        : dataValue),
    };
  }

  if (typeof result === 'undefined') {
    return {
      success: successWhenUndefined,
      data: null,
    };
  }

  return {
    success: true,
    data: typeof dataValue === 'undefined' ? result : dataValue,
  };
}

export function unwrapCompatResponseData(result, fallback) {
  if (result && typeof result === 'object' && !Array.isArray(result)) {
    if (Object.prototype.hasOwnProperty.call(result, 'data')) {
      return result.data;
    }
    if (typeof result.success === 'boolean') {
      return fallback;
    }
  }
  return result;
}
