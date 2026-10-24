import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { fetchCandidates, fetchReadingVersion } from './dossierApi';
import { dossierKey } from './evidenceKeys';
import { hideCachedReading } from './dossierPresent';
import { ImageCompare } from './ImageCompare';
import { ReadingReviewPanel } from './ReadingReviewPanel';
import type { OcrRecognitionImage } from './ocrRecognitionTypes';
import { ApiError } from '../../../shared/api';

interface Props { image: OcrRecognitionImage; work: string; domain: string; onSaved: () => void }
/** OCR owns the entry point; all edits and decisions use the canonical reading command. */
export function OcrReadingReview(props: Props) {
  const [activated, setActivated] = useState(false);
  if (props.image.state !== 'succeeded' || props.image.candidateRevisionRef === null) return null;
  return <section aria-label="人工核对图片读本">
    {activated ? <ReviewTarget {...props} /> : <ActionButton onClick={() => setActivated(true)}>人工核对与采用</ActionButton>}
  </section>;
}
function ReviewTarget({ image, work, domain, onSaved }: Props) {
  const client = useQueryClient();
  const [originalReadable, setOriginalReadable] = useState(true);
  const context = useQuery({ queryKey: ['ocr-reading-review', domain, work, image.candidateRevisionRef], retry: false, gcTime: 0, queryFn: async ({ signal }) => {
    const [candidates, version] = await Promise.all([
      fetchCandidates(work, domain, signal), fetchReadingVersion(work, domain, image.candidateRevisionRef!, signal),
    ]);
    const candidate = candidates.find(value => value.candidateRevisionRef === image.candidateRevisionRef) ?? null;
    if (candidate && candidate.unitRef !== version.material.ref) throw new ApiError({ kind: 'invalid_response', status: 200 });
    // The server's exact-unit pointer is authoritative even when the dossier deduplicates this unit.
    // A requested immutable version is not itself proof of what is current.
    const current = version.currentRevisionRef;
    const selected = current === null || current === version.material.currentRevisionRef ? version : await fetchReadingVersion(work, domain, current, signal);
    if (selected.currentRevisionRef !== current || selected.material.ref !== version.material.ref) throw new ApiError({ kind: 'invalid_response', status: 200 });
    const unit = current === null ? { ...version.material, currentRevisionRef: null, reading: null, blocks: [], issues: [] } : selected.material;
    return { unit, candidate };
  } });
  function applied() {
    void context.refetch();
    void client.invalidateQueries({ queryKey: dossierKey(domain, work) });
    void client.invalidateQueries({ queryKey: ['dossier-candidates', domain, work] });
    onSaved();
  }
  if (hideCachedReading(context.error)) return <p role="alert">来源当前不可读，人工核对已暂停。<button type="button" onClick={() => void context.refetch()}>重新读取来源状态</button></p>;
  if (context.isPending) return <p role="status">正在读取这张图片的当前读本与候选…</p>;
  if (context.isError && !context.data) return <p role="status">未能确认这张图片的核对版本。<button type="button" onClick={() => void context.refetch()}>重试读取核对版本</button></p>;
  const { unit, candidate } = context.data;
  return <>
    <ActionButton variant="quiet" disabled={context.isFetching} onClick={() => void context.refetch()}>刷新核对版本</ActionButton>
    {context.isError ? <p role="status">刷新未完成，仍保留上次读取的版本。<button type="button" onClick={() => void context.refetch()}>重试刷新核对版本</button></p> : null}
    <ImageCompare key={unit.originalUrl} material={{ ...unit, blocks: candidate?.blocks ?? unit.blocks }} onReadable={setOriginalReadable} />
    <ReadingReviewPanel unit={unit} work={work} domain={domain} onApplied={applied} initialCandidate={candidate ?? undefined} initiallyOpen originalReadable={originalReadable} showCurrentReading />
  </>;
}
