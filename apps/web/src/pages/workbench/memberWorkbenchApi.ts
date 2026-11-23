import { ApiError, requestJson } from '../../shared/api';
import { ShapeError, bool, decode, field, list, obj, optObj, optStr, str, strList, type Json } from '../../shared/decode';
import { decodeExperiences, decodePersonaFacts, decodePublicationLinks, parseIdentity } from './assets/assetsModel';
import { parseIdeaBoxList, parseSaveReceipt, uuid, type WorkDocument } from './workbenchModel';

function accountPlatform(item: Json): "xhs" {
  const platform = str(item, 'platform');
  if (platform !== 'xhs') throw new ShapeError('platform');
  return platform;
}

const api = '/api/local/ops/creation';

function ownership(item: Json) {
  const value = str(item, 'ownership');
  if (value !== 'mine' && value !== 'team' && value !== 'unknown') throw new ShapeError('ownership');
  return value;
}

function parseWorkspace(value: unknown) {
  return decode(value, root => ({
    works: list(root, 'works', item => ({
      workId: uuid(str(item, 'workId')), title: str(item, 'title'), ownership: ownership(item),
      publishedRevisionId: optStr(item, 'publishedRevisionId'),
    })),
    ideas: parseIdeaBoxList({ items: field(root, 'ideas') }).items,
    accounts: list(root, 'accounts', item => ({
      ref: uuid(str(item, 'ref')), name: str(item, 'name'), platform: accountPlatform(item),
      canPublish: bool(item, 'canPublish'),
    })),
  }));
}

function parseDraft(value: unknown) {
  return decode(value, root => {
    const draft = obj(root, 'draft');
    return {
      workId: uuid(str(root, 'workId')), domainRef: uuid(str(root, 'domainRef')), revisionId: uuid(str(root, 'revisionId')),
      editable: bool(root, 'editable'), ownership: ownership(root),
      publications: decodePublicationLinks({ items: field(root, 'publications') }),
      document: {
        title: str(draft, 'title'), accountRef: optStr(draft, 'accountRef'), tags: strList(draft, 'tags'),
        paragraphs: list(draft, 'paragraphs', item => ({ id: uuid(str(item, 'id')), text: str(item, 'text') })),
      },
      restricted: bool(draft, 'hasRestrictedText'),
    };
  });
}

export type Workspace = ReturnType<typeof parseWorkspace>;
export type Draft = ReturnType<typeof parseDraft>;

export async function readDomains(signal?: AbortSignal) {
  return decode(await requestJson(`${api}/domains`, { signal }), root =>
    list(root, 'items', item => ({ domainRef: uuid(str(item, 'domainRef')), name: str(item, 'name') })));
}

export async function readWorkspace(domain: string, mine: boolean, signal?: AbortSignal) {
  return parseWorkspace(await requestJson(`${api}/works?${new URLSearchParams({ domainRef: domain, mine: String(mine) })}`, { signal }));
}

export async function readDraft(work: string, signal?: AbortSignal) {
  const draft = parseDraft(await requestJson(`${api}/works/${uuid(work)}`, { signal }));
  if (draft.workId !== work) throw new ShapeError('workId');
  return draft;
}

type DraftCommand = { requestId: string; document: WorkDocument };

export async function create(command: DraftCommand & { domainRef: string }) {
  const receipt = parseSaveReceipt(await requestJson(`${api}/works`, { method: 'POST', body: command }));
  if (receipt.requestId !== command.requestId || receipt.outcome !== 'accepted') throw new ShapeError('requestId');
  return receipt;
}

export async function save(work: string, command: DraftCommand & { expectedRevision: string }) {
  const receipt = parseSaveReceipt(await requestJson(`${api}/works/${uuid(work)}`, { method: 'POST', body: command }));
  if (receipt.requestId !== command.requestId || receipt.workId !== work) throw new ShapeError('requestId');
  if (receipt.outcome === 'stale') throw new ApiError({ kind: 'http', status: 409, code: 'creation_stale' });
  return receipt;
}

export async function readAccountAssets(account: string, signal?: AbortSignal) {
  return decode(await requestJson(`${api}/accounts/${uuid(account)}/assets`, { signal }), root => {
    const identity = optObj(root, 'identity');
    return {
      identity: identity ? parseIdentity(identity) : null,
      facts: decodePersonaFacts({ items: field(root, 'personaFacts') }),
      experiences: decodeExperiences({ items: field(root, 'experiences') }),
    };
  });
}
