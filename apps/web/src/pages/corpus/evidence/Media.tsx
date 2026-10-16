import { useState, type ReactNode } from 'react';
import { stateMeta } from './evidencePresent';
import styles from './Media.module.css';

type Phase = 'loading' | 'loaded' | 'failed';

interface MediaImageProps {
  /** 已通过同源受控句柄校验的地址；null 表示没有取得。 */
  url: string | null;
  /** 各阶段的可访问名称：必须和图片的真实状态一致（F04）。 */
  names: { loaded: string; loading: string; failed: string; absent: string };
  fallback: ReactNode;
  className: string | undefined;
}

function MediaImageInner({ url, names, fallback, className }: MediaImageProps) {
  const [phase, setPhase] = useState<Phase>('loading');
  const showImage = url !== null && phase !== 'failed';
  const name = url === null ? names.absent : phase === 'loaded' ? names.loaded : phase === 'failed' ? names.failed : names.loading;
  return (
    <span role="img" aria-label={name} className={`${styles.frame} ${className ?? ''}`} data-media-state={url === null ? 'absent' : phase}>
      {showImage ? <img className={styles.image} src={url} alt="" loading="lazy" decoding="async" onLoad={() => setPhase('loaded')} onError={() => setPhase('failed')} /> : null}
      {showImage && phase === 'loaded' ? null : <span className={styles.fallback}>{fallback}</span>}
    </span>
  );
}

/** 地址换了就是另一张图：用 key 重置加载状态，不让上一张的「已加载」留给下一张。 */
export function MediaImage(props: MediaImageProps) {
  return <MediaImageInner key={props.url ?? ''} {...props} />;
}

/** 作者头像。只在服务器标记为可内联的受控副本存在时显示图片；否则写「作」占位，名称写头像状态。 */
export function AuthorAvatar({ author, url, state }: { author: string; url: string | null; state: string }) {
  const word = stateMeta(state)[0];
  return (
    <MediaImage
      url={url}
      className={styles.avatar}
      fallback="作者"
      names={{ loaded: `${author}的头像`, loading: '头像读取中', failed: '头像读取失败', absent: `作者头像${word}` }}
    />
  );
}

/** 研读行预览：3:4，完整显示。没有受控封面时显示封面状态词。 */
export function CoverPreview({ title, url, state }: { title: string; url: string | null; state: string }) {
  const word = stateMeta(state)[0];
  return (
    <MediaImage
      url={url}
      className={styles.preview}
      fallback={word}
      names={{ loaded: `${title}的封面`, loading: '封面读取中', failed: '封面读取失败', absent: `封面${word}` }}
    />
  );
}
