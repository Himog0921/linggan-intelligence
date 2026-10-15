import { AppShell } from './AppShell';
import styles from './NotFoundPage.module.css';

/** 服务器把本应用交给了一个前端没有登记的路径。这是登记表不一致，不是用户操作错误。 */
export function NotFoundPage() {
  return (
    <AppShell
      active={null}
      boundaryLabel="本机服务状态"
      crumb={<b>页面不存在</b>}
      meta={<span>本机工作空间</span>}
    >
      <div className={`v7-shell ${styles.shell}`}>
        <main className={styles.main} aria-labelledby="not-found-title">
          <title>页面不存在 · Linggan Intelligence</title>
          <h1 id="not-found-title">这个地址没有对应的页面</h1>
          <p>新版网页里还没有登记这个地址。请从顶部导航进入需要的模块。</p>
        </main>
      </div>
    </AppShell>
  );
}
