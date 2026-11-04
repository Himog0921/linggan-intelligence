import type { DossierBlock } from './dossierTypes';

/** Matches content::has_source_text after canonical block-shape validation. */
export function hasReadingSourceText(block: DossierBlock): boolean {
  switch (block.kind) {
    case 'gap': return false;
    case 'heading': case 'paragraph': case 'quote': return block.text.trim() !== '';
    // Their canonical text includes list markers, table separators or dialogue attribution.
    case 'list': case 'table': case 'dialogue': return true;
  }
}
