export const archiveLabels: Record<string, string> = {
  not_applicable: '不适用',
  not_started: '尚未建立',
  queued: '已排队',
  running: '正在执行',
  partial: '部分可用',
  blocked: '有待处理项',
  complete: '已建立',
  unavailable: '当前读不到',
};
export const executionLabels: Record<string, string> = {
  idle: '当前无任务执行',
  queued: '已排队，等待调度',
  awaiting_producer: '已分配，等待执行工位',
  running: '正在执行',
  blocked: '执行已阻塞',
};
export const patrolLabels: Record<string, string> = {
  disabled: '未开启',
  waiting: '已开启，等待首次结果',
  queued: '已排队',
  awaiting_producer: '等待执行工位',
  running: '正在巡查',
  normal: '运行正常',
  partial: '最近巡查未完成，已取得的内容保留',
  blocked: '巡查已阻塞',
};
export const rankings: Record<string, string> = {
  primary: '创作者主页',
  most_liked: '最多点赞',
  most_collected: '最多收藏',
  most_commented: '最多评论',
  latest: '最新排序',
  comprehensive: '综合排序',
};
