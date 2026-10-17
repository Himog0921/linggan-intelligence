import { useCallback, useState } from 'react';
import { PANELS, type PanelWidth } from './evidenceQuery';

/** 检查器宽度是界面偏好，不含任何作品内容。 */
const STORAGE_KEY = 'linggan.evidence.inspectorWidth';

function read(): PanelWidth {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    return PANELS.find(width => width === stored) ?? 'normal';
  } catch {
    // 隐私窗口或站点数据被屏蔽不得弄坏面板。
    return 'normal';
  }
}

export function usePanelWidthPreference(): readonly [PanelWidth, (width: PanelWidth) => void] {
  const [width, setWidth] = useState<PanelWidth>(read);
  const remember = useCallback((next: PanelWidth) => {
    setWidth(next);
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      /* 同上。 */
    }
  }, []);
  return [width, remember] as const;
}
