import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { ApiError } from '../../../shared/api';
import { COLUMN_LABELS, fetchNoteMaterial, MATERIAL_FIELDS, MaterialContractError, materialReason, type MaterialColumn, type NoteMaterial } from './noteMaterial';
import { FactGrid, Section } from './Primitives';
import styles from './Dossier.module.css';

const FIELD_LABELS = { title: '标题', cover_headline: '封面大字', body: '正文', tags: '话题标签', ocr: '图片文字', transcript: '转写' };
const SCOPE_LABELS: Readonly<Record<string, string>> = {
  image_ocr: '图片文字识别', asr: '语音转写', comment_image: '评论图片', author_avatar: '作者头像',
  thumbnail: '缩略图', audio_extract: '提取音轨', video_frame_ocr: '视频帧文字识别',
};

function Column({ column, material }: { column: MaterialColumn; material: NoteMaterial }) {
  const restricted = column.state === 'restricted' || column.reasons.some(reason => reason.split(':').at(-1) === 'source_restricted');
  const fragments = restricted ? [] : material.fragments.filter(fragment => fragment.field === column.field);
  return (
    <div>
      <h4>{FIELD_LABELS[column.field]}</h4>
      <p>{COLUMN_LABELS[column.state]}</p>
      {column.reasons.length ? <ul>{column.reasons.map(reason => <li key={reason}>{materialReason(reason)}</li>)}</ul> : null}
      {restricted ? <p>本栏来源受限，文字不在此显示。</p> : fragments.map(fragment => (
        <details key={fragment.citationId} className={styles.disclosure}>
          <summary>{`${fragment.citationId} · ${Array.from(fragment.text).length} 字符 · 查看片段与来源`}</summary>
          <p className={styles.text}>{fragment.text}</p>
          <FactGrid rows={[
            ['来源引用', fragment.sourceRef], ['来源版本', fragment.sourceVersion], ['逻辑位置', fragment.logicalPosition],
            ['Unicode 字符范围（末端不含）', `${fragment.start}–${fragment.end}`], ['文字哈希', fragment.textHash],
          ]} />
        </details>
      ))}
      {!restricted && fragments.length === 0 ? <p className={styles.note}>本次读取没有可显示的片段。</p> : null}
    </div>
  );
}

function MaterialView({ material }: { material: NoteMaterial }) {
  const byField = new Map(material.columns.map(column => [column.field, column]));
  const transcript = byField.get('transcript');
  const scopeText = (values: string[]) => values.length ? values.map(value => SCOPE_LABELS[value]).join('、') : '本次未列出';
  return (
    <>
      <p role="status">{`底稿准备状态：${material.ready ? '材料已达到后端准备条件' : '材料仍有待处理项'}`}</p>
      {material.notReadyReasons.length ? <ul>{material.notReadyReasons.map(reason => <li key={reason}>{materialReason(reason)}</li>)}</ul> : null}
      <p className={styles.note}>准备状态来自后端，包含底稿处理范围；不表示材料完整、已分析或获准发送给模型。</p>
      <p>默认每篇输入预览：以下五列。当前仅供只读核对。</p>
      {MATERIAL_FIELDS.filter(field => field !== 'transcript').map(field => {
        const column = byField.get(field);
        return column ? <Column key={field} column={column} material={material} /> : null;
      })}
      <details className={styles.disclosure}>
        <summary>转写上下文（未纳入默认每篇输入）</summary>
        {transcript ? <Column column={transcript} material={material} /> : null}
      </details>
      <details className={styles.disclosure}>
        <summary>读取身份与底稿处理范围</summary>
        <p className={styles.note}>读取身份用于核对本次响应；没有保存快照。底稿处理范围不代表模型发送许可。</p>
        <FactGrid rows={[
          ['领域', material.domainRef], ['作品', material.workRef], ['读取截止时间', material.asOf],
          ['底稿修订', material.dossierRevision], ['材料指纹', material.materialFingerprint],
          ['来源哈希', material.provenanceHash], ['读取身份', material.snapshotIdentity], ['分层版本', material.layerVersion],
          ['底稿已纳入的处理', scopeText(material.processingScope.included)],
          ['仅作上下文', scopeText(material.processingScope.contextOnly)], ['本期未接入', scopeText(material.processingScope.notConnected)],
        ]} />
      </details>
    </>
  );
}

function ReadMaterial({ work, domain }: { work: string; domain: string }) {
  // This component exists only while expanded. It has no persistent cache or previous-data fallback.
  const query = useQuery({ queryKey: ['note-material-preview', domain, work], queryFn: ({ signal }) => fetchNoteMaterial(work, domain, signal), gcTime: 0, staleTime: 0 });
  let failure = '读取未成功，请重试。本区未保留上次正文。';
  if (query.error instanceof ApiError) {
    if (query.error.failure.kind === 'invalid_response') failure = '读取合同待核对，暂不显示正文；请重试或联系维护者。';
    else if (query.error.failure.kind === 'http' && query.error.failure.status === 404) failure = '当前领域或作品范围无法读取每篇材料；这不表示原作品没有材料。';
    else if (query.error.failure.kind === 'http' && query.error.failure.status === 403) failure = '当前材料读取受限，本区不显示正文。';
  }
  return (
    <div aria-busy={query.isFetching}>
      <button type="button" disabled={query.isFetching} onClick={() => void query.refetch()}>刷新每篇材料</button>
      {query.isFetching || query.isPending ? <p role="status">正在只读核对材料…</p>
        : query.isError ? <p role="status">{failure}</p>
          : <MaterialView material={query.data} />}
      {!query.isFetching && query.isError && query.error instanceof MaterialContractError ? <details className={styles.disclosure}><summary>合同诊断</summary><code>{query.error.diagnostic}</code></details> : null}
    </div>
  );
}

/** Keyed by work/domain at the dossier boundary: switching scope cannot reveal an old response. */
export function NoteMaterialSection({ work, domain }: { work: string; domain: string }) {
  const [expanded, setExpanded] = useState(false);
  return (
    <Section title="每篇解读材料">
      <p className={styles.note}>核对这篇作品的准备状态、缺口和可引用文字；只读，不启动研究或处理材料。</p>
      <details className={styles.disclosure} onToggle={event => setExpanded(event.currentTarget.open)}>
        <summary>展开每篇解读材料</summary>
        {expanded ? <ReadMaterial work={work} domain={domain} /> : null}
      </details>
    </Section>
  );
}
