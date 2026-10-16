import { collectionReturnHref, creatorsReturnHref, hasCreatorScope, type EvidenceQuery } from './evidenceQuery';
import styles from './EvidencePage.module.css';

/**
 * 两个返回横幅。来源由地址决定：带 `creatorKey` 或 `creatorFilter` 是创作者作品集合；带 `collectionRunRef`
 * 是本次采集的固定作品。返回目标只接受同源的指定页面前缀，否则回到那个页面本身。
 * 「当前为创作者作品筛选集合」「返回创作者」「返回采集任务」是其他页面与脚本依赖的文字，保持不变。
 */
export function ReturnBanners({ query, domainParam }: { query: EvidenceQuery; domainParam: string | null }) {
  return (
    <>
      {query.collectionRunRef !== null ? (
        <p className={styles.banner}>
          当前为本次采集的固定作品；详情与评论按本次取得／明确复用版本回查；媒体为共享当前资源。{' '}
          <a href={collectionReturnHref(query, domainParam)}>返回采集任务</a>
        </p>
      ) : null}
      {hasCreatorScope(query) ? (
        <p className={styles.banner}>
          当前为创作者作品筛选集合 · <a href={creatorsReturnHref(query, domainParam)}>返回创作者</a>
        </p>
      ) : null}
    </>
  );
}
