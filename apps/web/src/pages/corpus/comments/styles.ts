import page from './CommentResearch.module.css';

/** Keep the B-04 class names local to this route, including dialog portals. Unknown names pass through (global v7-*). */
export function c(...names: (string | false | null | undefined)[]) {
  return names.flatMap(name => name ? name.split(/\s+/) : []).filter(Boolean)
    .map(name => page[name] ?? name).join(' ');
}
