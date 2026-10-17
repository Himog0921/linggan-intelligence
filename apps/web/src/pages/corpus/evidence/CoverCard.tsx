import { useId, useState, type KeyboardEvent, type MouseEvent } from 'react';
import { GEOMETRY_SHAPES, coverGeometryKind, coverKindLabel, type GeometryKind } from './coverGeometry';
import { StateLine } from './Primitives';
import {
  coverObservationMoment,
  coverPublishedCopy,
  laneSummary,
  materialSummary,
  segmentTitle,
  stateMeta,
  workTitle,
} from './evidencePresent';
import type { WorkItem } from './evidenceTypes';
import { NO_HOVER_QUERY, matches } from './useMediaQuery';
import { CheckBox, type RowProps } from './WorkRow';
import { AuthorFact, Engagement } from './WorkParts';
import styles from './CoverCard.module.css';

/** 线稿只是目录标记：不推断内容含义、证据状态或质量。 */
function Geometry({ kind }: { kind: GeometryKind }) {
  return (
    <svg className={styles.geometry} viewBox="0 0 300 400" aria-hidden="true" focusable="false">
      <g fill="none" stroke="currentColor" strokeWidth="1.25">
        {GEOMETRY_SHAPES[kind].map((shape, index) => {
          if (shape.tag === 'circle') return <circle key={index} cx={shape.cx} cy={shape.cy} r={shape.r} />;
          if (shape.tag === 'rect') return <rect key={index} x={shape.x} y={shape.y} width={shape.width} height={shape.height} />;
          return <path key={index} d={shape.d} />;
        })}
      </g>
    </svg>
  );
}

function Face({ front, item, title, index }: { front: boolean; item: WorkItem; title: string; index: string }) {
  const kind = coverGeometryKind(item);
  const cover = item.media.coverUrl;
  return (
    <section className={`${styles.face} ${front ? styles.front : styles.back}`} aria-hidden={front ? undefined : 'true'}>
      <div className={styles.faceInner}>
        <header className={styles.faceHead}>
          {front ? <h2 className={styles.coverTitle} title={title}>{title}</h2> : <span className={styles.backLabel}>原始封面</span>}
          <span className={styles.index}>{index}</span>
        </header>
        <div className={styles.stage}>
          <div className={`${styles.stageFrame} ${front ? styles.diagram : styles.photo}`} data-unavailable={!front && cover === null}>
            {front ? (
              <Geometry kind={kind} />
            ) : cover !== null ? (
              <img src={cover} alt={`${title}封面`} loading="lazy" decoding="async" />
            ) : (
              <strong className={styles.unavailable}>{stateMeta(item.media.coverState)[0]}</strong>
            )}
          </div>
        </div>
        <footer className={styles.faceFoot}>
          <span className={styles.micro}>{front ? `${coverKindLabel(kind)} · 已观察` : '受控本地副本'}</span>
          <span className={styles.cue}>{front ? '查看封面 →' : '3:4'}</span>
        </footer>
      </div>
    </section>
  );
}

/**
 * 翻转卡的正面（线稿）与背面（原始封面）。支持悬停的设备上鼠标悬停即翻（纯 CSS）；触屏点击卡面翻转且不触发
 * 行选择；键盘 Enter／Space 翻转。翻转是旋转，不是上浮；减少动态效果时直接切换。
 */
function Visual({ item, title, index }: { item: WorkItem; title: string; index: string }) {
  const [flipped, setFlipped] = useState(false);
  const toggle = (event: { preventDefault: () => void; stopPropagation: () => void }) => {
    event.preventDefault();
    event.stopPropagation();
    setFlipped(value => !value);
  };
  const onClick = (event: MouseEvent) => {
    if (matches(NO_HOVER_QUERY)) toggle(event);
  };
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === 'Enter' || event.key === ' ') toggle(event);
  };
  return (
    <div
      className={styles.visual}
      role="button"
      tabIndex={0}
      aria-pressed={flipped}
      aria-label={`${title}，${flipped ? '收起封面' : '查看封面'}`}
      data-flipped={flipped}
      onClick={onClick}
      onKeyDown={onKeyDown}
    >
      <div className={styles.flip}>
        {/* 两个面共用的隐形占位：用满宽 3:4 舞台推出卡片高度，而不是先定一个宽卡高度再让竖版封面悬在空白里。 */}
        <div className={styles.sizer} aria-hidden="true">
          <span className={styles.sizerTitle} />
          <span className={styles.sizerStage}>
            <span className={styles.sizerFrame} />
          </span>
          <span className={styles.sizerFoot} />
        </div>
        <Face front item={item} title={title} index={index} />
        <Face front={false} item={item} title={title} index={index} />
      </div>
    </div>
  );
}

/** 五维状态：铭牌上的悬停提示，同一份文字也作为选择控件的补充说明给读屏。评论维度写「保留数 / 平台数」。 */
function statusLines(item: WorkItem): string[] {
  const summary = materialSummary(item);
  const comments = laneSummary(item.lanes, 'comments');
  const platform = item.display.engagement.comment;
  return summary.segments.map(segment => {
    if (segment.key === 'comments' && comments?.valueState === 'KNOWN' && comments.retained !== null && platform.state === 'KNOWN' && platform.value !== null) {
      return `评论 ${comments.retained.toLocaleString('zh-CN')} / ${platform.value.toLocaleString('zh-CN')}`;
    }
    return segmentTitle(segment);
  });
}

/** 封面排版的一张卡：翻转面＋常驻铭牌。铭牌上盖一个隐形的选择按钮，给键盘与读屏用；鼠标点整张卡都选中。 */
export function CoverCard({ item, ordinal, selected, tabStop, canCheck, checked, onSelect, onCheck }: RowProps & { ordinal: number }) {
  const statusId = useId();
  const title = workTitle(item);
  const detail = laneSummary(item.lanes, 'detail')?.state ?? 'UNKNOWN';
  const lines = statusLines(item);
  return (
    <li className={styles.item} data-selected={selected} data-platform={item.platform.toLowerCase()} data-work-ref={item.ref} onClick={() => onSelect(item)}>
      <div className={styles.card}>
        <Visual item={item} title={title} index={String(ordinal).padStart(3, '0')} />
        <div className={styles.plate}>
          <button type="button" className={styles.select} data-select-target="" aria-current={selected ? 'true' : undefined} tabIndex={tabStop ? 0 : -1} aria-label={title} aria-describedby={statusId} />
          <div className={styles.plateTop}>
            <AuthorFact item={item} />
            <span className={styles.date} title={`发布时间 ${coverPublishedCopy(item.display)}`}>
              {coverPublishedCopy(item.display)}
            </span>
          </div>
          <Engagement engagement={item.display.engagement} />
          <div className={styles.facts}>
            <span className={styles.status}>
              <StateLine state={detail} />
              <span className={styles.tooltip} aria-hidden="true">
                {lines.map(line => (
                  <span key={line}>{line}</span>
                ))}
              </span>
            </span>
            <span className={styles.observed} title={`最近观察 ${item.lastObservedAt ?? '未知'}`}>
              {coverObservationMoment(item.lastObservedAt)}
            </span>
          </div>
          <span id={statusId} className="v7-sr-only">
            {lines.join('；')}
          </span>
        </div>
        {canCheck ? <CheckBox item={item} checked={checked} onCheck={onCheck} /> : null}
      </div>
    </li>
  );
}
