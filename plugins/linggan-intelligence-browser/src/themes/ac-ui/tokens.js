/**
 * AC-UI 主题 Token — 供 content script / page injection 使用
 * 完全还原 animal-crossing-ui 源码风格
 */

// 6 组马卡龙色系（从源码 colorUtils.js 提取）
export const AC_COLORS = {
  red:    ['#C14953', '#CD5D67', '#DAA49A', '#FFBCB5', '#FBBFCA', '#C99DA3', '#EDB6A3'],
  orange: ['#E88873', '#F19A3E', '#D0A98F', '#EABDA8', '#F34213', '#FC814A', '#E5B25D', '#F7A278', '#F49D37'],
  yellow: ['#F6E27F', '#E2C391', '#F5CB5C', '#F9DF74', '#F2DD6E', '#FFCB47', '#E6AF2E'],
  green:  ['#C2FBEF', '#7DAA92', '#ABEDC6', '#98D9C2', '#B9FFB7', '#BFD7B5', '#A3C4BC', '#9CBFA7', '#BAD9B5'],
  blue:   ['#86BBD8', '#C1DBE3', '#C0E8F9', '#66C3FF', '#5C80BC', '#95B8D1', '#C2EFEB', '#9AC2C9'],
  khaki:  ['#F2E7C9', '#EDD4B2', '#CAC2B5', '#ECDCC9', '#C2A878', '#E2C290', '#F9EDCC', '#BCAB79'],
};

// 常用固定色
export const TOKENS = {
  theme: '#2fb1ff',
  bg: '#f2f2f2',
  faint: '#f8f8f8',
  text: '#4f4f4f',
  dialogText: '#4a4d56',
  radius: '5px',
  baseUnit: '6px',
};

/** 从色系中随机取一个颜色 */
export function pickColor(type = 'blue') {
  const colors = AC_COLORS[type] || AC_COLORS.blue;
  return colors[Math.floor(Math.random() * colors.length)];
}

/** 生成 AC-UI 按钮的内联样式字符串 */
export function buttonStyle(colorType = 'blue', { ghost = false } = {}) {
  const color = pickColor(colorType);
  if (ghost) {
    return `border:1px solid ${color};border-radius:${TOKENS.radius};background:#fff;color:${TOKENS.text};`;
  }
  return (
    `border:1px solid ${color};` +
    `border-radius:${TOKENS.radius};` +
    `color:${TOKENS.text};` +
    `background-image:linear-gradient(to right, ${color}66, white);` +
    `background-size:200% 100%;` +
    `background-position:right 0 top 0;` +
    `transition:background-position 300ms ease, transform 150ms ease;` +
    `cursor:pointer;`
  );
}

/** 任务控制台外壳样式 */
export function taskbarShellStyle(width = 320) {
  return {
    position: 'fixed',
    right: '20px',
    bottom: '24px',
    zIndex: '2147483646',
    width: `${width}px`,
    minHeight: '182px',
    padding: '12px',
    display: 'none',
    boxSizing: 'border-box',
    background: '#f2f2f2',
    border: '1px solid #ddd',
    borderRadius: '5px',
    boxShadow: '-20px 80px 80px -80px rgba(0,0,0,0.25)',
    color: TOKENS.text,
    fontFamily: "'Segoe UI', 'PingFang SC', 'Microsoft YaHei', sans-serif",
  };
}

/** Toast 样式 */
export function toastStyle(type = 'info') {
  const toneMap = {
    info:    { bg: '#C0E8F9', border: '#86BBD8' },
    success: { bg: '#ABEDC6', border: '#7DAA92' },
    warning: { bg: '#F6E27F', border: '#E2C391' },
    error:   { bg: '#FFBCB5', border: '#C14953' },
  };
  const tone = toneMap[type] || toneMap.info;
  return {
    position: 'fixed',
    top: '20px',
    left: '50%',
    transform: 'translateX(-50%)',
    width: '320px',
    minHeight: '52px',
    boxSizing: 'border-box',
    padding: '10px 16px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    textAlign: 'center',
    background: tone.bg,
    color: TOKENS.text,
    border: `1px solid ${tone.border}`,
    borderRadius: '5px',
    boxShadow: '0 4px 12px rgba(0,0,0,0.08)',
    fontSize: '13px',
    fontWeight: '700',
    lineHeight: '1.35',
    zIndex: '2147483646',
    transition: 'opacity 0.3s',
    fontFamily: "'Segoe UI', 'PingFang SC', 'Microsoft YaHei', sans-serif",
    whiteSpace: 'normal',
  };
}
