/**
 * 整页跳转到仍由旧服务提供的页面（采集页）。单独成函数，是为了让测试能在不真正离开页面的情况下
 * 核对跳转地址。
 */
export function assignLocation(url: string): void {
  window.location.assign(url);
}
