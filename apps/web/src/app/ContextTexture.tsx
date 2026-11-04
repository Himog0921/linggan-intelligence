import { lazy, Suspense, useState } from 'react';
import styles from './ContextTexture.module.css';

// React Bits Pattern Waves: preserve the upstream OGL/WebGL2 shader (see license).
// The "pond" preset is the recorded origin; explicit single-field values win.
const PatternWaves = lazy(() => import('../shared/patternWaves/PatternWaves'));

const CONTEXT_PATTERN_SETTINGS = {
  preset: 'pond',
  pattern: 'square',
  wave: 'silk',
  characters: '.:-=+*#%@',
  spacing: 6,
  markSize: 0.5,
  depth: 0.35,
  light: -23,
  shine: 0.94,
  contrast: 1.2,
  speed: 0.42,
  scale: 0.85, // Studio "scale: 85" -> shader scale 0.85.
  direction: 35,
  opacity: 1,
  fade: 'none',
  fadeSize: 0.5,
  interactive: true,
  cursorStrength: 0.5,
  cursorSize: 160,
  color: '#ffffff',
  backgroundColor: '#120f17',
} as const;

/**
 * One visual context-strip material for every standard AppShell route.
 * The shader paints behind existing breadcrumbs, menus and view tabs.
 * WebGL2 is optional; unsupported/reduced-motion environments retain a static
 * dark strip, rather than a visually different homegrown animation.
 */
export function ContextTexture() {
  const [canRender] = useState(() => {
    if (typeof window === 'undefined' || typeof document === 'undefined') return false;
    if (typeof WebGL2RenderingContext === 'undefined') return false;
    if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return false;
    if (window.matchMedia?.('(max-width: 820px)').matches) return false;
    try {
      const gl = document.createElement('canvas').getContext('webgl2');
      if (!gl) return false;
      gl.getExtension('WEBGL_lose_context')?.loseContext();
      return true;
    } catch {
      return false;
    }
  });

  return (
    <div
      className={styles.backdrop}
      data-context-pattern="square"
      data-context-preset="pond"
      data-context-wave="silk"
      aria-hidden="true"
    >
      {canRender ? (
        <div className={styles.stage}>
          <Suspense fallback={null}>
            <PatternWaves {...CONTEXT_PATTERN_SETTINGS} intro={false} paused={false} />
          </Suspense>
        </div>
      ) : null}
      <span className={styles.veil} />
    </div>
  );
}
