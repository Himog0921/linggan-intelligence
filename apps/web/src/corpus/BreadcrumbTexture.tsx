import { lazy, Suspense, useState } from 'react';
import styles from './BreadcrumbTexture.module.css';

// Keep OGL/GLSL out of unrelated routes, and out of the initial lightweight page shell.
const PatternWaves = lazy(() => import('../shared/patternWaves/PatternWaves'));

type GrainPattern = 'dot' | 'square';

/** Preview-only URL switch; no control is inserted into the production breadcrumb.
 * /corpus/creators?...&breadcrumbPattern=square shows the same Silk material with a square lattice.
 * Without the query the original dot reference remains the baseline.
 */
function queryPattern(): GrainPattern {
  if (typeof window === 'undefined') return 'dot';
  return new URLSearchParams(window.location.search).get('breadcrumbPattern') === 'square' ? 'square' : 'dot';
}

export function BreadcrumbTexture() {
  const pattern = queryPattern();
  // GPU capability is an initial environment property, not a React effect.
  // A static dark strip is preferable to a low-fidelity 2D imitation.
  const [canRender] = useState(() => {
    if (typeof WebGL2RenderingContext === 'undefined') return false;
    if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return false;
    if (window.matchMedia?.('(max-width: 820px)').matches) return false;
    try {
      const probe = document.createElement('canvas').getContext('webgl2');
      if (!probe) return false;
      probe.getExtension('WEBGL_lose_context')?.loseContext();
      return true;
    } catch {
      return false;
    }
  });

  return (
    <div className={styles.backdrop} data-pattern={pattern} aria-hidden="true">
      {canRender ? (
        <div className={styles.stage}>
          <Suspense fallback={null}>
            <PatternWaves
              preset="silk"
              pattern={pattern}
              wave="silk"
              spacing={10}
              markSize={0.6}
              depth={0.35}
              light={-23}
              color="#ffffff"
              backgroundColor="#120f17"
              fade="none"
              opacity={1}
              interactive={false}
              intro={false}
            />
          </Suspense>
        </div>
      ) : null}
      <span className={styles.veil} />
    </div>
  );
}
