const INITIAL_STATE_ASSIGNMENT = 'window.__INITIAL_STATE__=';
const NOTE_DETAIL_MAP_KEY = '"noteDetailMap"';
const MAX_SSR_SCRIPT_LENGTH = 5 * 1024 * 1024;
const cache = new WeakMap();

function readBalancedJsonObject(text, objectStart) {
  if (text[objectStart] !== '{') return '';

  let depth = 0;
  let inString = false;
  let escaped = false;

  for (let index = objectStart; index < text.length; index += 1) {
    const character = text[index];
    if (inString) {
      if (escaped) {
        escaped = false;
      } else if (character === '\\') {
        escaped = true;
      } else if (character === '"') {
        inString = false;
      }
      continue;
    }

    if (character === '"') {
      inString = true;
    } else if (character === '{') {
      depth += 1;
    } else if (character === '}') {
      depth -= 1;
      if (depth === 0) return text.slice(objectStart, index + 1);
      if (depth < 0) return '';
    }
  }

  return '';
}

function parseNoteDetailMap(scriptText) {
  const text = String(scriptText || '');
  if (
    !text
    || text.length > MAX_SSR_SCRIPT_LENGTH
    || !text.includes(INITIAL_STATE_ASSIGNMENT)
  ) {
    return null;
  }

  const keyIndex = text.indexOf(NOTE_DETAIL_MAP_KEY);
  if (keyIndex < 0) return null;
  const colonIndex = text.indexOf(':', keyIndex + NOTE_DETAIL_MAP_KEY.length);
  if (colonIndex < 0) return null;
  const objectStart = text.indexOf('{', colonIndex + 1);
  if (objectStart < 0) return null;

  const serializedMap = readBalancedJsonObject(text, objectStart);
  if (!serializedMap) return null;

  try {
    const parsed = JSON.parse(serializedMap);
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

/**
 * XHS removes the global initial state after hydration on some detail routes, while the
 * original SSR script remains in the document. Parse only the serialized noteDetailMap;
 * never evaluate or execute page script text.
 */
export function readXhsSsrNoteDetailMap(doc = globalThis.document) {
  if (!doc || typeof doc !== 'object') return {};
  const cached = cache.get(doc);
  if (cached) return cached;

  for (const script of Array.from(doc.scripts || [])) {
    const noteMap = parseNoteDetailMap(script?.textContent);
    if (noteMap) {
      cache.set(doc, noteMap);
      return noteMap;
    }
  }

  return {};
}
