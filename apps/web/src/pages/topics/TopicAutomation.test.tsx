import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { TopicAutomation } from './TopicAutomation';
const json = (value: unknown) => new Response(JSON.stringify(value), { headers: { 'Content-Type': 'application/json' } });
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
it('reads and changes the domain setting using its server revision', async () => {
  const fetch = vi.fn().mockResolvedValueOnce(json({ domainRef: 'one', enabled: false, revision: 3 }))
    .mockResolvedValueOnce(json({ domainRef: 'one', enabled: true, revision: 4 }));
  vi.stubGlobal('fetch', fetch);
  render(<TopicAutomation domain="one" />);
  const button = await screen.findByRole('switch');
  expect(button).toHaveAttribute('aria-checked', 'false');
  fireEvent.click(button);
  await waitFor(() => expect(button).toHaveAttribute('aria-checked', 'true'));
  expect(JSON.parse(fetch.mock.calls[1]![1].body as string)).toMatchObject({ domainRef: 'one', expectedRevision: 3, enabled: true });
});
it('reads back an unknown response without assuming failure', async () => {
  const fetch = vi.fn().mockResolvedValueOnce(json({ domainRef: 'one', enabled: false, revision: 0 }))
    .mockRejectedValueOnce(new TypeError('offline'))
    .mockResolvedValueOnce(json({ domainRef: 'one', enabled: true, revision: 1 }));
  vi.stubGlobal('fetch', fetch);
  render(<TopicAutomation domain="one" />);
  fireEvent.click(await screen.findByRole('switch'));
  await waitFor(() => expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true'));
  expect(fetch).toHaveBeenCalledTimes(3);
});

it('recovers a failed initial read in the open panel', async () => {
  const fetch = vi.fn().mockRejectedValueOnce(new TypeError('offline'))
    .mockResolvedValueOnce(json({ domainRef: 'one', enabled: true, revision: 2 }));
  vi.stubGlobal('fetch', fetch);
  render(<TopicAutomation domain="one" />);
  fireEvent.click(await screen.findByRole('button', { name: '重试' }));
  expect(await screen.findByRole('switch')).toHaveAttribute('aria-checked', 'true');
  expect(screen.queryByRole('status')).not.toBeInTheDocument();
});

it('shows an enabled setting waiting for configuration without blocking the switch', async () => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(json({ domainRef: 'one', enabled: true, revision: 1, configurationReady: false })));
  render(<TopicAutomation domain="one" />);
  expect(await screen.findByRole('status')).toHaveTextContent('待配置');
  expect(screen.getByRole('switch')).not.toBeDisabled();
});
