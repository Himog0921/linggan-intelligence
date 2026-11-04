import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { MaterialTiles } from './MaterialTiles';
import { materialSummary } from './evidencePresent';
import { parseWorkList } from './evidenceDecode';
import { listItemJson, listJson } from './evidenceFixtures';

it('one disclosure preserves channel facts and closes without selecting its row', async () => {
  const select = vi.fn();
  const item = parseWorkList(listJson([listItemJson(1, { media: { images: 1 } })])).items[0]!;
  const user = userEvent.setup();
  render(<div onClick={select}><MaterialTiles summary={materialSummary(item)} /></div>);
  const trigger = screen.getByRole('button', { name: /^材料说明：/ });
  expect(screen.getAllByRole('button')).toHaveLength(1);
  expect(trigger).toHaveTextContent('正评媒图转');
  await user.click(trigger);
  const dialog = screen.getByRole('dialog', { name: '材料说明' });
  for (const name of ['正文', '评论', '媒体', '图片文字', '转录']) expect(within(dialog).getByText(name)).toBeInTheDocument();
  expect(within(dialog).getByText('本作品不适用')).toBeInTheDocument();
  fireEvent.keyDown(document, { key: 'Escape' });
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();
  await user.click(trigger);
  await user.click(screen.getByRole('button', { name: '关闭' }));
  expect(trigger).toHaveFocus();
  expect(select).not.toHaveBeenCalled();
});

it('outside pointer closes the explanation', async () => {
  const item = parseWorkList(listJson([listItemJson(1)])).items[0]!;
  const user = userEvent.setup();
  render(<><MaterialTiles summary={materialSummary(item)} /><button>外部</button></>);
  await user.click(screen.getByRole('button', { name: /^材料说明：/ }));
  await user.click(screen.getByRole('button', { name: '外部' }));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
});


it('Tab and Shift+Tab leave the portal in the trigger’s local keyboard order', async () => {
  const item = parseWorkList(listJson([listItemJson(1)])).items[0]!;
  const user = userEvent.setup();
  render(<><button>前一项</button><MaterialTiles summary={materialSummary(item)} /><button>后一项</button></>);
  const trigger = screen.getByRole('button', { name: /^材料说明：/ });
  await user.click(trigger);
  await user.tab();
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: '后一项' })).toHaveFocus();
  await user.click(trigger);
  await user.tab({ shift: true });
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: '前一项' })).toHaveFocus();
});
