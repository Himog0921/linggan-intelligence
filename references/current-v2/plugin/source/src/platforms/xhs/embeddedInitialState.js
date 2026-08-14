const INITIAL_STATE_PREFIX = 'window.__INITIAL_STATE__=';
const NOTE_MAP_KEY = '"noteDetailMap"';
const MAX_EMBEDDED_STATE_LENGTH = 5 * 1024 * 1024;

const documentCache = new WeakMap();

function findBalancedJsonObject(source, startIndex) {
  if (source[startIndex] !== '{') return '';
  let depth = 0;
  let inString = false;
  let escaped = false;

  for (let index = startIndex; index < source.length; index += 1) {
    const char = source[index];
    if (inString) {
      if (escaped) {
        escaped = false;
      } else if (char === '\\') {
        escaped = true;
      } else if (char === '"') {
        inString = false;
      }
      continue;
    }

    if (char === '"') {
      inString = true;
      continue;
    }
    if (char === '{') depth += 1;
    if (char !== '}') continue;
    depth -= 1;
    if (depth === 0) return source.slice(startIndex, index + 1);
    if (depth < 0) return '';
  }
  return '';
}

function readNoteMapFromScriptText(text) {
  if (!text || text.length > MAX_EMBEDDED_STATE_LENGTH || !text.includes(INITIAL_STATE_PREFIX)) return null;
  const keyIndex = text.indexOf(NOTE_MAP_KEY);
  if (keyIndex < 0) return null;
  const colonIndex = text.indexOf(':', keyIndex + NOTE_MAP_KEY.length);
  if (colonIndex < 0) return null;
  const objectStart = text.indexOf('{', colonIndex + 1);
  if (objectStart < 0) return null;
  const objectJson = findBalancedJsonObject(text, objectStart);
  if (!objectJson) return null;
  try {
    const parsed = JSON.parse(objectJson);
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

export function readEmbeddedXhsNoteDetailMap(documentLike = globalThis.document) {
  if (!documentLike || typeof documentLike !== 'object') return {};
  const cached = documentCache.get(documentLike);
  if (cached) return cached;

  const scripts = Array.from(documentLike.scripts || []);
  for (const script of scripts) {
    const noteMap = readNoteMapFromScriptText(String(script?.textContent || ''));
    if (!noteMap) continue;
    documentCache.set(documentLike, noteMap);
    return noteMap;
  }
  return {};
}
