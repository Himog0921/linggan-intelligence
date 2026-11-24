import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { AccountPicker } from './AccountPicker';

const accounts = [{ ref: 'a', name: '裴裴妈妈' }, { ref: 'b', name: '小满老师' }, { ref: 'c', name: '小满同学' }];

function open(value: string | null = null, onPick = vi.fn()) {
  render(<><AccountPicker accounts={accounts} value={value} mark={value ? '裴' : '未'} name={value ? '裴裴妈妈' : '未选择账号'} onPick={onPick} /><p>别处</p></>);
  return onPick;
}

it('shows the account as a button, closed until it is clicked', async () => {
  open();
  expect(screen.getByRole('button', { name: '写作账号：未选择账号' })).toHaveAttribute('aria-expanded', 'false');
  expect(screen.queryByRole('group', { name: '选择写作账号' })).toBeNull();
});

it('opens with the cursor in the search field and every account listed', async () => {
  const user = userEvent.setup();
  open();
  await user.click(screen.getByRole('button', { name: '写作账号：未选择账号' }));
  expect(screen.getByLabelText('搜索账号')).toHaveFocus();
  const list = within(screen.getByRole('group', { name: '选择写作账号' }));
  expect(list.getAllByRole('button', { pressed: false }).map(button => button.textContent)).toEqual(['裴裴妈妈', '小满老师', '小满同学']);
});

it('narrows the list as the name is typed, in Chinese too', async () => {
  const user = userEvent.setup();
  open();
  await user.click(screen.getByRole('button', { name: '写作账号：未选择账号' }));
  await user.type(screen.getByLabelText('搜索账号'), '小满');
  const list = within(screen.getByRole('group', { name: '选择写作账号' }));
  expect(list.queryByRole('button', { name: '裴裴妈妈' })).toBeNull();
  expect(list.getAllByRole('button', { pressed: false })).toHaveLength(2);
  await user.type(screen.getByLabelText('搜索账号'), '老');
  expect(list.getAllByRole('button', { pressed: false }).map(button => button.textContent)).toEqual(['小满老师']);
});

it('says so when nothing matches, instead of showing an empty box', async () => {
  const user = userEvent.setup();
  open();
  await user.click(screen.getByRole('button', { name: '写作账号：未选择账号' }));
  await user.type(screen.getByLabelText('搜索账号'), '没有这个号');
  expect(screen.getByText('没有匹配“没有这个号”的账号。')).toBeInTheDocument();
});

it('picks the account that was clicked, closes, and gives the focus back to the button', async () => {
  const user = userEvent.setup();
  const onPick = open();
  await user.click(screen.getByRole('button', { name: '写作账号：未选择账号' }));
  await user.click(screen.getByRole('button', { name: '小满老师' }));
  expect(onPick).toHaveBeenCalledExactlyOnceWith('b');
  expect(screen.queryByRole('group', { name: '选择写作账号' })).toBeNull();
  expect(screen.getByRole('button', { name: '写作账号：未选择账号' })).toHaveFocus();
});

it('picks the one account left by the search when Enter is pressed', async () => {
  const user = userEvent.setup();
  const onPick = open();
  await user.click(screen.getByRole('button', { name: '写作账号：未选择账号' }));
  await user.type(screen.getByLabelText('搜索账号'), '同学{Enter}');
  expect(onPick).toHaveBeenCalledExactlyOnceWith('c');
});

it('does not choose for the user when several accounts still match and Enter is pressed', async () => {
  const user = userEvent.setup();
  const onPick = open();
  await user.click(screen.getByRole('button', { name: '写作账号：未选择账号' }));
  await user.type(screen.getByLabelText('搜索账号'), '小满{Enter}');
  expect(onPick).not.toHaveBeenCalled();
  expect(screen.getByRole('group', { name: '选择写作账号' })).toBeInTheDocument();
});

it('marks the current account and does not report a pick of the account that is already set', async () => {
  const user = userEvent.setup();
  const onPick = open('a');
  await user.click(screen.getByRole('button', { name: '写作账号：裴裴妈妈' }));
  const current = within(screen.getByRole('group', { name: '选择写作账号' })).getByRole('button', { name: '裴裴妈妈' });
  expect(current).toHaveAttribute('aria-pressed', 'true');
  await user.click(current);
  expect(onPick).not.toHaveBeenCalled();
});

it('lets a draft go back to having no account, only when it has one', async () => {
  const user = userEvent.setup();
  const onPick = open('a');
  await user.click(screen.getByRole('button', { name: '写作账号：裴裴妈妈' }));
  await user.click(screen.getByRole('button', { name: '不指定账号' }));
  expect(onPick).toHaveBeenCalledExactlyOnceWith(null);
});

it('does not offer 不指定账号 to a draft that has no account', async () => {
  const user = userEvent.setup();
  open();
  await user.click(screen.getByRole('button', { name: '写作账号：未选择账号' }));
  expect(screen.queryByRole('button', { name: '不指定账号' })).toBeNull();
});

it('closes on Escape and on a click anywhere else, without picking', async () => {
  const user = userEvent.setup();
  const onPick = open();
  const button = screen.getByRole('button', { name: '写作账号：未选择账号' });
  await user.click(button);
  await user.keyboard('{Escape}');
  expect(screen.queryByRole('group', { name: '选择写作账号' })).toBeNull();
  expect(button).toHaveFocus();
  await user.click(button);
  await user.click(screen.getByText('别处'));
  expect(screen.queryByRole('group', { name: '选择写作账号' })).toBeNull();
  expect(onPick).not.toHaveBeenCalled();
});

it('starts every opening with an empty search', async () => {
  const user = userEvent.setup();
  open();
  const button = screen.getByRole('button', { name: '写作账号：未选择账号' });
  await user.click(button);
  await user.type(screen.getByLabelText('搜索账号'), '小满');
  await user.keyboard('{Escape}');
  await user.click(button);
  expect(screen.getByLabelText('搜索账号')).toHaveValue('');
});

it('explains an empty domain', async () => {
  const user = userEvent.setup();
  render(<AccountPicker accounts={[]} value={null} mark="未" name="未选择账号" onPick={() => undefined} />);
  await user.click(screen.getByRole('button', { name: '写作账号：未选择账号' }));
  expect(screen.getByText('这个领域还没有可用的账号。')).toBeInTheDocument();
});
