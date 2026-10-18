import { decode, obj, optObj, optStr, str, type Json } from '../../../shared/decode';

export function parseSourceContext(row: Json) {
  return decode(row, r => { const key = obj(r, 'commentKey'), comment = optObj(r, 'comment'), work = optObj(r, 'work'), parent = optObj(r, 'parentContext'); return { commentKey: { workRef: str(key, 'workRef'), commentExternalId: str(key, 'commentExternalId') }, sourceState: optStr(obj(r, 'source'), 'sourceState'), commentText: comment ? optStr(comment, 'commentText') : null, workTitle: (work ? optStr(work, 'displayTitle') : null) || (comment ? optStr(comment, 'workTitle') : null), parentText: parent ? optStr(parent, 'commentText') : null }; });
}
