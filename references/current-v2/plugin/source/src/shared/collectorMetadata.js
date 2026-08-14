export const COLLECTOR_VERSION = '2026-03-27-wave5-v1';

function truncateText(value = '', max = 6000) {
  const text = String(value || '').trim();
  if (!text) return '';
  return text.length > max ? `${text.slice(0, max)}...` : text;
}

function safeSerialize(value) {
  if (value == null) return '';
  try {
    return truncateText(JSON.stringify(value));
  } catch {
    return truncateText(String(value));
  }
}

export function joinRawDomText(parts = []) {
  const list = Array.isArray(parts) ? parts : [parts];
  return truncateText(
    list
      .map((item) => String(item || '').trim())
      .filter(Boolean)
      .join('\n\n'),
  );
}

export function createCollectorEvidence({
  rawPayload = null,
  rawDomText = '',
  rawShareText = '',
  rawUrl = '',
  rawSource = '',
} = {}) {
  return {
    collectorVersion: COLLECTOR_VERSION,
    rawPayload: safeSerialize(rawPayload),
    rawDomText: truncateText(rawDomText),
    rawShareText: truncateText(rawShareText),
    rawUrl: String(rawUrl || '').trim(),
    rawSource: String(rawSource || '').trim(),
  };
}

export function createCollectorQualityMeta({
  dataQuality = 'full',
  qualityReason = '',
  sourceTier = '',
} = {}) {
  return {
    dataQuality: String(dataQuality || 'full').trim() || 'full',
    qualityReason: String(qualityReason || '').trim(),
    sourceTier: String(sourceTier || '').trim(),
  };
}
