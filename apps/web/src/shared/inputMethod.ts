/** Text inputs match :focus-visible after pointer clicks in Chromium as well.
 * Track modality explicitly so approved C can distinguish pointer from keyboard. */
export function trackInputMethod(doc: Document): () => void {
  doc.documentElement.dataset.inputMode = 'keyboard';
  const pointer = () => { doc.documentElement.dataset.inputMode = 'pointer'; };
  const keyboard = (event: KeyboardEvent) => {
    if (!event.metaKey && !event.ctrlKey && !event.altKey && !['Shift','Control','Alt','Meta'].includes(event.key)) doc.documentElement.dataset.inputMode = 'keyboard';
  };
  doc.addEventListener('pointerdown', pointer, true);
  doc.addEventListener('keydown', keyboard, true);
  return () => { doc.removeEventListener('pointerdown', pointer, true); doc.removeEventListener('keydown', keyboard, true); };
}
