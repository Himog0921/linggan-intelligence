const momentFormat = new Intl.DateTimeFormat('zh-CN', { timeZone: 'Asia/Shanghai', year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hourCycle: 'h23' });
export function studyTime(value: string | null | undefined) {
  const date = new Date(value ?? '');
  if (Number.isNaN(date.valueOf())) return '时间未记录';
  const part = Object.fromEntries(momentFormat.formatToParts(date).map(({ type, value: text }) => [type, text]));
  return `${part.year}-${part.month}-${part.day} ${part.hour}:${part.minute}`;
}
export const num = (value: number | null | undefined) => value == null || !Number.isFinite(value) ? '—' : new Intl.NumberFormat('zh-CN').format(value);
export const shortRef = (value: string | null | undefined) => value?.slice(0, 8).toUpperCase() || '未记录';
export const errorText = (error: unknown) => error instanceof Error ? error.message : '读取未完成';

/** Blank detection never trims readable source or saved research text. */
export const nonBlank = (value: string | null | undefined) => value?.trim() ? value : '';
