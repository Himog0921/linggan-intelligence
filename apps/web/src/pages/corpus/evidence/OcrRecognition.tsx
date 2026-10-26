import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { readOcrRecognition, startOcrRecognition, stopOcrRecognition } from './ocrRecognitionApi';
import type { OcrRecognitionRun, OcrRecognitionScope } from './ocrRecognitionTypes';
import { Section } from './Primitives';
import styles from './OcrRecognition.module.css';

function ocrPercent(run: OcrRecognitionRun): number {
  if (run.state === 'succeeded') return 100;
  return run.totalImages <= 0 ? 0 : Math.min(99, Math.floor(run.completedImages / run.totalImages * 100));
}
const isLive = (run: OcrRecognitionRun | null) => Boolean(run && ['queued', 'running', 'finalizing', 'unknown'].includes(run.state));
interface StopRequest { commandRef: string; batchRef: string; expectedRevision: number }
function actionCopy(run: OcrRecognitionRun | null) {
  if (!run || run.state === 'ready' || run.state === 'unavailable') return 'OCR 识别';
  if (run.cancelRequested && isLive(run)) return '正在停止';
  if (run.state === 'queued') return '已排队 0%';
  if (run.state === 'running') return '识别中 ' + ocrPercent(run) + '%';
  if (run.state === 'finalizing') return '整理结果 ' + ocrPercent(run) + '%';
  if (run.state === 'succeeded') return '识别完成 100%';
  if (run.state === 'stopped') return '已停止 ' + ocrPercent(run) + '%';
  return '核对结果中';
}
function stageCopy(run: OcrRecognitionRun) {
  if (run.state === 'unavailable') return run.unavailableReason ?? '识别资格尚未确认，请核对 OCR 设置。';
  if (run.state === 'succeeded') {
    const published = run.images.filter(image => image.state === 'succeeded' && image.readingRef !== null && image.isCurrent).length;
    return published > 0 ? `识别已完成，${published} 张图片已进入当前底稿。${published < run.completedImages ? '其余识别结果未替换当前底稿。' : ''}` : '识别已完成，当前底稿未替换。';
  }
  if (run.state === 'unknown') return '结果暂未确认；只核对原任务，不重复发送图片。';
  if (run.state === 'stopped') {
    const reasons: Record<string, string> = {
      ocr_visual_coordinates_invalid: '文字位置无效',
      ocr_visual_title_mismatch: '标题与识别原文不一致',
      ocr_visual_schema_invalid: '识别结果格式不完整',
      ocr_output_truncated: '识别输出不完整',
      ocr_provider_failed: '识别服务未返回有效结果',
    };
    const reason = run.failureCode ? reasons[run.failureCode] : undefined;
    const published = run.images.filter(image => image.state === 'succeeded' && image.readingRef !== null && image.isCurrent).length;
    return `${reason ? `${reason}，本次已停止` : '本次已停止'}。${published > 0 ? `${published} 张图片已进入当前底稿。` : ''}后续图片未发送。`;
  }
  if (run.cancelRequested) return '正在核对停止回执；在途请求只尽力中断，已得结果保留。';
  if (run.state === 'queued') return '已排队，等待已启用的执行器处理。';
  if (run.state === 'finalizing') return '正在提交可读结果，尚未宣布全部完成。';
  if (run.currentStage) return run.currentStage;
  return run.unavailableReason ?? '保存模型不会自动发图。';
}
/** Parent keys this work-scoped session, so late A responses cannot appear in B. */
export function OcrRecognition({ workRef, active = true, onApplied, children, readingRevision }: { workRef: string; active?: boolean; readingRevision?: string; onApplied?: () => void; children?: ReactNode }) {
  const [run, setRun] = useState<OcrRecognitionRun | null>(null);
  const [loading, setLoading] = useState(true); const [readFailed, setReadFailed] = useState(false);
  const [sending, setSending] = useState(false); const [message, setMessage] = useState('');
  const [unconfirmed, setUnconfirmed] = useState(false);
  const [confirm, setConfirm] = useState<OcrRecognitionScope | null>(null);
  const [selectedSlots, setSelectedSlots] = useState<string[]>([]);
  const [refresh, setRefresh] = useState(0);
  const [stopRequest, setStopRequest] = useState<StopRequest | null>(null);
  const write = useRef<{ commandRef: string; scope: OcrRecognitionScope; slotKeys: string[] } | null>(null);
  const stopInFlight = useRef(false); const mounted = useRef(true);
  const result = useRef<HTMLDivElement>(null);
  const notified = useRef<string | null>(null);
  useEffect(() => {
    if (!run || !['succeeded', 'stopped'].includes(run.state) || !run.runRef) return;
    const key = `${run.runRef}:${run.state}:${run.completedImages}`;
    if (notified.current !== key) { notified.current = key; onApplied?.(); }
  }, [run, onApplied]);
  useEffect(() => {
    mounted.current = true; if (!active) return; const controller = new AbortController(); let timer: number | undefined;
    const load = async () => {
      try {
        const next = await readOcrRecognition(workRef, controller.signal);
        if (controller.signal.aborted) return;
        setRun(next); setReadFailed(false); setLoading(false);
        if (isLive(next)) timer = window.setTimeout(() => void load(), 1500);
      } catch {
        if (controller.signal.aborted) return;
        setReadFailed(true); setLoading(false); setMessage('状态暂未读取成功，请核对原任务后再操作。');
      }
    };
    void load();
    return () => { mounted.current = false; controller.abort(); window.clearTimeout(timer); };
  }, [workRef, refresh, active, readingRevision]);
  const value = run ? ocrPercent(run) : 0; const live = isLive(run);
  const canStart = !loading && !readFailed && !sending && !unconfirmed && stopRequest === null && !live && run !== null && ['ready', 'succeeded', 'stopped'].includes(run.state) && run.scope !== null;
  function chooseScope() { if (canStart && run?.scope) { setSelectedSlots(run.scope.images.map(image => image.slotKey)); setConfirm(run.scope); } }
  async function submit(scope: OcrRecognitionScope) {
    if (!write.current && selectedSlots.length === 0) return;
    if (sending || stopRequest !== null || stopInFlight.current) return;
    const pending = write.current ?? { commandRef: crypto.randomUUID(), scope, slotKeys: [...selectedSlots] }; write.current = pending;
    setSending(true); setMessage('');
    try {
      const next = await startOcrRecognition(workRef, pending.commandRef, pending.scope.sha256, pending.slotKeys);
      if (!mounted.current) return;
      write.current = null; setUnconfirmed(false); setRun(next); setConfirm(null); setRefresh(value => value + 1);
    } catch (error) {
      if (!mounted.current) return;
      if (error instanceof ApiError && error.failure.kind === 'http' && [400, 401, 403, 404, 409, 410, 413, 415, 422].includes(error.failure.status)) { write.current = null; setUnconfirmed(false); setConfirm(null); setMessage('本篇范围或运行条件已变化，请核对最新状态。'); setRefresh(value => value + 1); }
      else { setUnconfirmed(true); setConfirm(null); setMessage('发起回执暂未确认；“核对本次请求”使用原命令，不会另建任务。'); }
    } finally { if (mounted.current) setSending(false); }
  }
  async function stop() {
    if (sending || stopInFlight.current) return;
    let request = stopRequest;
    if (!request) {
      if (loading || readFailed || !live || !run?.batchRef || run.batchRevision === null || run.cancelRequested) return;
      request = { commandRef: crypto.randomUUID(), batchRef: run.batchRef, expectedRevision: run.batchRevision };
      setStopRequest(request);
    }
    stopInFlight.current = true; setSending(true); setMessage('');
    try {
      await stopOcrRecognition(request.batchRef, request.expectedRevision, request.commandRef);
      if (mounted.current) { setStopRequest(null); setLoading(true); setMessage('已提交停止请求，正在核对结果。'); setRefresh(value => value + 1); }
    } catch (error) {
      if (!mounted.current) return;
      if (error instanceof ApiError && error.failure.kind === 'http' && [400, 401, 403, 404, 409, 410, 413, 415, 422].includes(error.failure.status)) {
        setStopRequest(null); setLoading(true);
        setMessage(error.failure.status === 409 ? '停止请求未被接纳，任务版本已变化；正在读取最新状态，之后可重新选择停止。' : '停止请求未被接纳；正在重新核对任务状态与操作资格。');
        setRefresh(value => value + 1);
      } else setMessage('停止回执尚未确认；“核对本次停止请求”使用原批次、原修订与原命令，核清前不能开始新识读。');
    } finally { stopInFlight.current = false; if (mounted.current) setSending(false); }
  }
  function act() {
    if (live || run?.state === 'succeeded' || run?.state === 'stopped') {
      const target = result.current; const inspector = target?.closest('aside');
      const head = inspector?.querySelector(':scope > header');
      if (target && inspector && head) inspector.scrollTo({ top: inspector.scrollTop + target.getBoundingClientRect().top - inspector.getBoundingClientRect().top - head.getBoundingClientRect().height, behavior: 'auto' });
      else target?.scrollIntoView({ block: 'start', behavior: 'auto' });
      return;
    }
    chooseScope();
  }
  const action = <><ActionButton className={styles.action} data-ocr-action data-state={run?.state ?? 'loading'} style={{ '--ocr-progress': value + '%' } as CSSProperties}
    disabled={loading || sending || (!live && run?.state !== 'succeeded' && run?.state !== 'stopped' && !canStart)} aria-busy={live || sending} onClick={act}>
    <i className={styles.fill} aria-hidden="true"/><span>{loading ? '读取状态中' : actionCopy(run)}</span>
  </ActionButton>{run?.runRef ? <span className={styles.progressSemantics} role="progressbar" aria-label="图片文字识别进度" aria-valuemin={0} aria-valuemax={100} aria-valuenow={value} aria-valuetext={value + '%'}/> : null}</>;
  return <Section title={children ? "图组" : "图片文字"} actions={action}>
    <div className={styles.module} data-ocr-recognition data-state={run?.state ?? 'loading'}>
      <div ref={result} className={styles.feedbackRow}><p className={styles.status} role="status" aria-live="polite">{run ? stageCopy(run) : loading ? '正在读取图片文字识别状态…' : '尚未取得识别资格。'}</p>
        <a href="/settings/models?view=advanced&area=work&task=ocr_recognition">OCR 模型设置</a>
        {run && ['succeeded', 'stopped'].includes(run.state) ? <button type="button" className={styles.source} disabled={!canStart} onClick={chooseScope}>选择图片重新识读</button> : null}
        {stopRequest ? <button type="button" className={styles.source} disabled={sending} onClick={() => void stop()}>核对本次停止请求</button> : live && run?.batchRef ? <button type="button" className={styles.source} disabled={loading || readFailed || sending || run.cancelRequested} onClick={() => void stop()}>停止本次识别</button> : null}</div>
      {run && ['succeeded', 'stopped'].includes(run.state) && run.scope === null && run.unavailableReason ? <p className={styles.status}>当前无法发起新识读：{run.unavailableReason}</p> : null}
      {message ? <p className={styles.status} role="status">{message}</p> : null}
      {readFailed ? <button className={styles.source} type="button" onClick={() => setRefresh(value => value + 1)}>重新读取状态</button> : null}
      {unconfirmed && !sending ? <button className={styles.source} type="button" onClick={() => { if (write.current) void submit(write.current.scope); }}>核对本次请求</button> : null}
      {confirm ? <div className={styles.confirm} role="group" aria-label="确认本篇图片识别范围"><b>重新识读 {selectedSlots.length} 张原图</b><p>将按既有源顺序发送到 {confirm.modelLabel}。每张一次，首个失败后停止；可能产生模型费用。</p><small>{confirm.endpointUrl}</small><fieldset disabled={sending}><legend>选择重新识读的图片</legend>{confirm.images.map(image => <label key={image.slotKey}><input type="checkbox" checked={selectedSlots.includes(image.slotKey)} onChange={event => setSelectedSlots(current => event.target.checked ? [...current, image.slotKey] : current.filter(key => key !== image.slotKey))} />{image.ordinal === null ? image.role === 'cover' ? '封面 · 页码未知' : '页码未知的图片' : `第 ${image.ordinal} 页`}</label>)}</fieldset>
        <div className={styles.confirmActions}><ActionButton variant="primary" disabled={sending || selectedSlots.length === 0} onClick={() => void submit(confirm)}>确认并开始识别</ActionButton><ActionButton disabled={sending} onClick={() => setConfirm(null)}>取消</ActionButton></div></div> : null}
      {children}
    </div>
  </Section>;
}
