/**
 * 给人看的时间只有一种写法：`YYYY-MM-DD HH:MM`，北京时间。与数据库函数
 * `linggan_human_moment`（迁移 0062，HUMAN-MOMENT-001）一致。机器合同仍用 ISO，不经过这里。
 */
const momentParts = new Intl.DateTimeFormat('en-CA', {
  timeZone: 'Asia/Shanghai',
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
  hourCycle: 'h23',
});

// PostgreSQL `timestamptz::text` 形如 `2026-10-15 14:30:00.123456+08`，浏览器不保证能解析，先规整成 ISO。
const postgresMoment =
  /^(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2})(?::(\d{2})(?:\.(\d+))?)?(Z|[+-]\d{2}(?::?\d{2})?)$/;

function toIso(value: string): string | null {
  const match = postgresMoment.exec(value.trim());
  if (!match) return null;
  const [, day, minute, second = '00', fraction = '', offset = ''] = match;
  const millis = fraction ? `.${fraction.slice(0, 3).padEnd(3, '0')}` : '';
  let zone = offset;
  if (offset !== 'Z') {
    const digits = offset.replace(':', '');
    zone = `${digits.slice(0, 3)}:${digits.slice(3, 5).padEnd(2, '0')}`;
  }
  return `${day}T${minute}:${second}${millis}${zone}`;
}

export function humanMoment(value: string | null | undefined): string {
  if (value === null || value === undefined || value.trim() === '') return '未知';
  const iso = toIso(value);
  // 认不出来就原样显示：编一个时间出来比显示一个看不懂的字符串更糟。
  if (iso === null) return value;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return value;
  const parts = Object.fromEntries(momentParts.formatToParts(date).map(part => [part.type, part.value]));
  return `${parts.year}-${parts.month}-${parts.day} ${parts.hour}:${parts.minute}`;
}

/** 数量：未知不显示成 0。 */
export function countOrUnknown(value: number | null | undefined): string {
  return value === null || value === undefined ? '未知' : String(value);
}
