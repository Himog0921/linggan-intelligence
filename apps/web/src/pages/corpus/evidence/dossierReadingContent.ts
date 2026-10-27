import type { DossierBlock, DossierCoverage, DossierMaterial } from './dossierTypes';

/** Matches content::has_source_text after canonical block-shape validation. */
export function hasReadingSourceText(block: DossierBlock): boolean {
  switch (block.kind) {
    case 'gap': return false;
    case 'heading': case 'paragraph': case 'quote': return block.text.trim() !== '';
    // Their canonical text includes list markers, table separators or dialogue attribution.
    case 'list': case 'table': case 'dialogue': return true;
  }
}
/** A validation/range set only. Actual in-document gap positions belong to the owner. */
export function missingImageTextPages(materials: readonly DossierMaterial[], inventory: DossierCoverage['imageInventory']): number[] {
  const pages = new Set(inventory.missingPages);
  for (const unit of materials) {
    if (unit.kind === 'image' && unit.page !== null && !['known_empty', 'not_applicable'].includes(unit.sourceState) && !unit.blocks.some(hasReadingSourceText)) pages.add(unit.page);
  }
  return [...pages].sort((a, b) => a - b);
}
