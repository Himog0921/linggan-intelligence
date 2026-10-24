import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError, requestJson } from '../../shared/api';
import { lookupWorkRequest, save, readWork } from './workbenchApi';
import { newDocument, WorkbenchContractError } from './workbenchModel';
vi.mock('../../shared/api', async () => {
  const actual = await vi.importActual<typeof import('../../shared/api')>('../../shared/api');
  return {
    ...actual,
    requestJson: vi.fn(),
    requestBinaryJson: vi.fn()
  };
});
const id = '00000000-0000-4000-8000-000000000001',
  other = '00000000-0000-4000-8000-000000000002';
const receipt = {
  requestId: id,
  workId: id,
  outcome: 'accepted',
  replayed: false,
  revision: {
    id,
    number: 1,
    savedAt: 'now',
    document: newDocument('原样保留😀')
  }
};
beforeEach(() => vi.clearAllMocks());
describe('workbench command receipt identity', () => {
  it('checks request and work identities before accepting save feedback', async () => {
    vi.mocked(requestJson).mockResolvedValue({
      ...receipt,
      requestId: other
    });
    await expect(save(id, {
      requestId: id,
      expectedRevision: id,
      document: newDocument()
    })).rejects.toThrow(WorkbenchContractError);
    expect(requestJson).toHaveBeenCalledTimes(1);
  });
  it('reads an uncertain request without issuing another command and preserves the original payload', async () => {
    vi.mocked(requestJson).mockResolvedValue({
      requestId: id,
      kind: 'work',
      receipt
    });
    expect(await lookupWorkRequest(id, id)).toEqual(receipt);
    expect(requestJson).toHaveBeenCalledWith(`/ops/api/creation/requests/${id}`);
  });
  it('treats not-found receipt as notyetconfirmed and refuses unrelated read owner', async () => {
    vi.mocked(requestJson).mockRejectedValueOnce(new ApiError({
      kind: 'http',
      status: 404,
      code: 'creation_not_found'
    }));
    expect(await lookupWorkRequest(id)).toBeNull();
    vi.mocked(requestJson).mockResolvedValue({
      workId: other,
      domainRef: id,
      savedBy: 'local_machine',
      revision: receipt.revision,
      sourceStatuses: [],
      exports: []
    });
    await expect(readWork(id)).rejects.toThrow(WorkbenchContractError);
  });
});
it('turns an identity-checked stale query receipt into the same explicit conflict as HTTP 409', async () => {
  const stale = {
    ...receipt,
    outcome: 'stale',
    replayed: true
  };
  vi.mocked(requestJson).mockResolvedValue({
    requestId: id,
    kind: 'work',
    receipt: stale
  });
  const error = await lookupWorkRequest(id, id).catch(error => error);
  expect(error).toBeInstanceOf(ApiError);
  const {
    conflictReceipt
  } = await import('./workbenchApi');
  expect(conflictReceipt(error)).toEqual(stale);
  vi.mocked(requestJson).mockResolvedValue({
    requestId: id,
    kind: 'work',
    receipt: {
      ...stale,
      requestId: other
    }
  });
  await expect(lookupWorkRequest(id, id)).rejects.toThrow(WorkbenchContractError);
});
describe('human failure copy preserves diagnostics', () => {
  it('keeps unknown technical codes out of primary feedback without discarding the API failure', async () => {
    const {
      errorText
    } = await import('./workbenchApi');
    const error = new ApiError({
      kind: 'http',
      status: 503,
      code: 'internal_unknown_engine_failure'
    });
    expect(errorText(error)).toBe('操作未完成，当前输入仍保留。请稍后重试。');
    expect(error.failure).toEqual({
      kind: 'http',
      status: 503,
      code: 'internal_unknown_engine_failure'
    });
    expect(errorText(new Error('opaque_backend_error'))).not.toContain('opaque_backend_error');
  });
  it('explains why an old work is preserved but cannot be opened under the current citation contract', async () => {
    const { errorText } = await import('./workbenchApi');
    expect(errorText(new ApiError({
      kind: 'http',
      status: 409,
      code: 'creation_legacy_reference_unavailable'
    }))).toContain('原稿已保留');
  });
  it('explains known AI failures and unknown states in Chinese with a next step', async () => {
    const {
      aiFailureText,
      aiStateText
    } = await import('./workbenchApi');
    expect(aiFailureText('writing_output_invalid')).toContain('正文没有改变');
    expect(aiFailureText('writing_output_invalid')).toContain('重新准备');
    expect(aiFailureText('future_backend_code')).toContain('当前输入仍保留');
    expect(aiFailureText('future_backend_code')).not.toContain('future_backend_code');
    expect(aiStateText('cancel_requested')).toContain('正在取消');
    expect(aiStateText('future_state')).toBe('结果暂未确认，请刷新查看');
  });
});

describe('bounded collection requests', () => {
  const collection = {contract:'content-workbench-materials.v1',asOf:'now',methodVersion:'workbench.lexical-context.v1',query:'作业启动',terms:['作业','启动'],state:'insufficient',gaps:['暂无评论'],documents:[],nextCursor:null,searchScope:'local_only'};
  it('prepares from explicit intent without issuing save, collection or model commands', async () => {
    const {prepareMaterials} = await import('./workbenchApi');
    vi.mocked(requestJson).mockResolvedValue(collection);
    const body = {domainRef:id,title:'第一步变小',angle:'作业启动',outline:['降低启动门槛'],terms:['作业','启动']};
    expect(await prepareMaterials(body)).toEqual(collection);
    expect(requestJson).toHaveBeenCalledExactlyOnceWith('/ops/api/creation/sources/prepare',{method:'POST',body,signal:undefined});
  });
  it('passes the source kind to server pagination instead of filtering a returned page', async () => {
    const {searchMaterials} = await import('./workbenchApi');
    vi.mocked(requestJson).mockResolvedValue(collection);
    await searchMaterials({domainRef:id,query:'作业',kind:'note',cursor:24});
    const url = new URL(String(vi.mocked(requestJson).mock.calls[0]?.[0]), 'http://127.0.0.1');
    expect(url.searchParams.get('kind')).toBe('note');
    expect(url.searchParams.get('cursor')).toBe('24');
    expect(requestJson).toHaveBeenCalledTimes(1);
  });
  it('does not mistake unconfirmed material lookup for a successful collection', async () => {
    const {resolveMaterials} = await import('./workbenchApi');
    vi.mocked(requestJson).mockRejectedValue(new ApiError({kind:'network'}));
    await expect(resolveMaterials({domainRef:id,selections:[]})).rejects.toThrow(ApiError);
    expect(requestJson).toHaveBeenCalledTimes(1);
  });
});
