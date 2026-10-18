import study from './Comments.module.css';
import results from './Results.module.css';

/** Keep the current surface's CSS names local to this route, including dialog portals. */
export function c(...names: string[]) {
  return names.flatMap(name => name.split(/\s+/)).filter(Boolean)
    .map(name => [...new Set([study[name], results[name]].filter(Boolean))].join(' ') || name).join(' ');
}
