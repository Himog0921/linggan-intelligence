import { useState, type ReactNode } from 'react';
import styles from './CreatorsPage.module.css';

type Phase = 'loading' | 'loaded' | 'failed';
type Kind = 'avatar' | 'cover';

/**
 * 可访问名称必须和图片的真实状态一致（F04）：地址还在读取、图片已显示、没有取得或加载失败，
 * 各有各的名字。旧页面图片已显示后容器还写着「头像未取得」，读屏会同时听到两个相反的说法。
 */
const NAMES: Record<Kind, { loaded: string; pending: string; absent: string }> = {
  avatar: { loaded: '作者头像', pending: '头像读取中', absent: '头像未取得' },
  cover: { loaded: '作品封面', pending: '封面读取中', absent: '封面未取得' },
};

interface MediaImageProps {
  kind: Kind;
  /** 已通过同源媒体路径校验的地址；null 表示没有取得。 */
  url: string | null;
  /** 地址本身还在读取（批量读取作品资源尚未返回）。 */
  pending: boolean;
  /** 图片没有显示时的占位：头像用名字首字，封面用符号。 */
  fallback: ReactNode;
  className: string | undefined;
}

function MediaImageInner({ kind, url, pending, fallback, className }: MediaImageProps) {
  const [phase, setPhase] = useState<Phase>('loading');
  const names = NAMES[kind];
  const showImage = url !== null && phase !== 'failed';
  const name = url === null ? (pending ? names.pending : names.absent) : phase === 'loaded' ? names.loaded : phase === 'failed' ? names.absent : names.pending;
  return (
    <span role="img" aria-label={name} className={className} data-media-state={url === null ? 'absent' : phase}>
      {showImage ? (
        <img
          className={styles.mediaImage}
          src={url}
          alt=""
          loading="lazy"
          onLoad={() => setPhase('loaded')}
          onError={() => setPhase('failed')}
        />
      ) : null}
      {showImage && phase === 'loaded' ? null : <span className={styles.mediaFallback}>{fallback}</span>}
    </span>
  );
}

/** 地址换了就是另一张图：用 key 重置加载状态，不让上一张的「已加载」留给下一张。 */
export function MediaImage(props: MediaImageProps) {
  return <MediaImageInner key={props.url ?? ''} {...props} />;
}

export function Avatar({ name, url, pending, className }: { name: string; url: string | null; pending: boolean; className: string | undefined }) {
  return <MediaImage kind="avatar" url={url} pending={pending} className={className} fallback={Array.from(name)[0] ?? '作'} />;
}

export function Cover({ url, pending, className }: { url: string | null; pending: boolean; className: string | undefined }) {
  return <MediaImage kind="cover" url={url} pending={pending} className={className} fallback="▧" />;
}
