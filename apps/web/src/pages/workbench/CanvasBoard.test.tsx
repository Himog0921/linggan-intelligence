import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { CanvasBoard } from './CanvasBoard';
import { GROUPS, newDocument, type CanvasBlock, type CanvasGroup } from './workbenchModel';
import { chooseOption } from '../../test/selectField';

const card = (group: CanvasGroup, id: string): CanvasBlock => ({ id, excerptId: null, group, note: '', expression: `我的表达 ${group}`, expressionRevision: 1 });
function show(canvas: CanvasBlock[]) {
  const onUpdate = vi.fn();
  render(<CanvasBoard document={{ ...newDocument(), canvas }} busy={false} onCompose={vi.fn()} onChange={vi.fn()} onUpdate={onUpdate}
    onLocate={vi.fn()} onPlace={vi.fn()} onMaterials={vi.fn()} onUse={() => ({ label: '', open: vi.fn() })} />);
  return { onUpdate };
}

describe('the older canvas with the sample\'s material roles', () => {
  it('has a lane for every role, named like the desk\'s picker', () => {
    show([]);
    expect(screen.getAllByRole('heading', { level: 2 }).map(node => node.firstChild?.textContent)).toEqual(GROUPS.map(item => item.label));
  });
  it('keeps a card in the lane of its role, so one set on the desk is not lost here', () => {
    const roles: CanvasGroup[] = ['resonance', 'experience', 'title_reference', 'reference_only', 'counterexample', 'argument'];
    show(roles.map((group, index) => card(group, `c${index}`)));
    expect(screen.getByText(`${roles.length} 张材料卡`)).toBeVisible();
    for (const group of roles) {
      const lane = screen.getByRole('heading', { level: 2, name: new RegExp(`^${GROUPS.find(item => item.id === group)!.label}`) }).closest('section')!;
      expect(within(lane).getByLabelText(new RegExp(`我的表达 c${roles.indexOf(group)}`))).toHaveValue(`我的表达 ${group}`);
    }
  });
  it('moves a card to another role from its own list, using the same names', () => {
    const { onUpdate } = show([card('inbox', 'c0')]);
    fireEvent.click(screen.getByText('备注与整理'));
    chooseOption('材料归组', '反例');
    expect(onUpdate).toHaveBeenCalledWith('c0', { group: 'counterexample' });
  });
});
