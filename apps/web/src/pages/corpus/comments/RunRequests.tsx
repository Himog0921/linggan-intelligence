import { useState } from 'react';
import { errorText, studyTime } from './format';
import { label, modelRequestStateLabel, requestStageLabel } from './labels';
import { parseRequestDetail, type ModelRequest } from './runDtos';
import { useStudyRead } from './studyApi';
import { c } from './styles';

function RequestSnapshot({ request }: { request: ModelRequest }) {
  const identity = request.modelIdentity;
  return <><p>请求 {request.invocationRef} · {requestStageLabel[request.stage || ''] || '未知阶段'} · {label(modelRequestStateLabel, request.state) || '状态未记录'} · {request.dispatched ? '已派发' : '未派发'}</p><p>模型：{identity?.modelId || '名称未记录'}</p><details><summary>查看冻结模型身份</summary><p>modelRef {identity?.modelRef || '未记录'} · connectionVersionRef {identity?.connectionVersionRef || '未记录'} · modelConfigRef {request.modelConfigRef || '未记录'}</p></details><p>创建于 {studyTime(request.createdAt)}{request.dispatchStartedAt ? ` · 派发于 ${studyTime(request.dispatchStartedAt)}` : ''}</p>
    {request.sourceState === 'restricted' ? <p className={c('study-restricted')}>来源当前受限，本次请求正文快照不再显示。</p> : request.sourceState === 'unavailable' ? <p className={c('study-restricted')}>来源或候选当前不可用，本次请求正文快照不再显示。</p> : request.sourceState !== 'known' ? <p className={c('study-restricted')}>来源状态未确认，本次请求正文快照不显示。</p> : request.recordingState !== 'recorded' || request.requestManifest == null ? <p className={c('study-detail-muted')}>历史请求的完整快照未记录。</p> : <><h4>本次冻结请求快照</h4><pre>{JSON.stringify(request.requestManifest, null, 2)}</pre></>}
  </>;
}
function RequestCard({ request, runRef, domain }: { request: ModelRequest; runRef: string; domain: string }) {
  const [open, setOpen] = useState(false);
  const usage = request.usageKnown ? `输入 ${Number(request.inputTokens)} · 输出 ${Number(request.outputTokens)} · 计费 ${Number(request.chargedTokens)} 词元` : `用量未知 · 预留 ${Number(request.reservedTokens)} 词元 · 保守计入 ${Number(request.chargedTokens)} 词元`;
  return <article className={c('study-request-card')}><header><strong>{requestStageLabel[request.stage || ''] || '未知阶段'}</strong><span>{label(modelRequestStateLabel, request.state) || '状态未记录'} · {request.dispatched ? '已派发' : '未派发'}</span></header><p>模型：{request.modelIdentity?.modelId || '名称未记录'}</p><p>{studyTime(request.createdAt)} · 尝试 {Number(request.attemptOrdinal || 0)}</p><p>{usage}{request.elapsedMs != null ? ` · ${request.elapsedMs} ms` : ''}</p>{request.failureCode ? <p>错误码：{request.failureCode}</p> : null}{request.invocationRef ? <><button type="button" className={c('study-link')} data-request-detail={request.invocationRef} aria-expanded={open} onClick={() => setOpen(value => !value)}>{open ? '收起本次请求详情' : '查看本次请求详情'}</button>{open ? <ScopedRequestDetail domain={domain} runRef={runRef} invocationRef={request.invocationRef} /> : <div className={c('study-request-detail')} hidden />}</> : <p className={c('study-detail-muted')}>请求编号未记录，无法读取完整快照。</p>}</article>;
}
function ScopedRequestDetail({ domain, runRef, invocationRef }: { domain: string; runRef: string; invocationRef: string }) {
  const query = useStudyRead(domain, `requests/${encodeURIComponent(invocationRef)}`, row => { const value = parseRequestDetail(row); if (value.invocationRef !== invocationRef || value.runRef !== runRef) throw new Error('请求详情回执与当前运行不一致。'); return value; });
  return <div className={c('study-request-detail')}>{query.isPending ? <p className={c('study-detail-muted')}>正在读取本次请求快照…</p> : query.isError ? <p className={c('study-restricted')}>请求详情读取失败：{errorText(query.error)}</p> : query.data ? <RequestSnapshot request={query.data} /> : null}</div>;
}
export function RunRequests({ requests, runRef, domain }: { requests: ModelRequest[]; runRef: string; domain: string }) {
  if (!requests.length) return <p className={c('muted')}>本次研究没有模型请求记录。</p>;
  return <>{requests.map((request, i) => <RequestCard request={request} runRef={runRef} domain={domain} key={request.invocationRef || i} />)}</>;
}
