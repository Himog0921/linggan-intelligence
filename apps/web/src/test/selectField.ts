import { fireEvent, screen, within } from '@testing-library/react';

const optionText = (node: HTMLElement) => (node.textContent ?? '').replace('✓', '').trim();

/** Opens a shared SelectField by its label and picks the option whose visible text equals (or matches) `option`. */
export function chooseOption(label: string | RegExp, option: string | RegExp) {
  fireEvent.click(screen.getByRole('combobox', { name: label }));
  const list = screen.getByRole('listbox', { name: label });
  const choice = within(list).getAllByRole('option').find(item => typeof option === 'string' ? optionText(item) === option : option.test(optionText(item)));
  if (!choice) throw new Error(`选项不存在：${String(option)}`);
  fireEvent.click(choice);
}

/** The text a SelectField currently shows. */
export function selectedText(label: string | RegExp): string {
  return screen.getByRole('combobox', { name: label }).textContent?.replace('⌄', '').trim() ?? '';
}
