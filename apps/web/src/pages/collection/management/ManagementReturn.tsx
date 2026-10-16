export function ManagementReturn({ returnTo, runtime = false }: { returnTo: string; runtime?: boolean }) {
  return <nav className="c-management-return" data-collection-management-context aria-label="返回采集工作页"><a className="c-btn c-btn-quiet" data-collection-management-return href={returnTo}>{returnTo.split(/[?#]/)[0] === '/collection/tasks' ? '返回采集任务' : '返回观察目标'}</a>{runtime && <span>工位是全局共享资源，当前领域仅用于返回工作页。</span>}</nav>;
}
