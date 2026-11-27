import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { OperatingDirections } from './OperatingDirections';
import { requestJson } from '../../shared/api';

vi.mock('../../shared/api', async (original) => ({
  ...(await original<typeof import('../../shared/api')>()),
  requestJson: vi.fn(),
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
it('previews changes before publishing and invalidates the preview after an edit', async () => {
  const draft = {
    directions: [
      {
        directionRef: 'd',
        label: '时间管理',
        definition: '日程安排',
        topicRefs: ['t'],
      },
    ],
    benchmarkAuthors: [],
  };
  vi.mocked(requestJson).mockImplementation(async (path) =>
    path.endsWith('/preview')
      ? {
          previewHash: 'abc',
          before: 0,
          after: 1,
          changedWorks: 1,
          priorDirections: 0,
          nextDirections: 1,
        }
      : {
          revision: 1,
          publishedRevision: null,
          totalWorks: 2,
          topics: [
            {
              id: 't',
              label: '时间管理',
              definition: '日程安排',
              definitionRef: 'v',
              samples: [],
              parent: null,
            },
          ],
          authors: [],
          draft,
          published: null,
        },
  );
  render(<OperatingDirections domain="adhd" />);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '查看运营方向' }));
  await user.click(await screen.findByRole('button', { name: '预览发布影响' }));
  expect(
    await screen.findByRole('button', { name: '确认并发布方向' }),
  ).toBeInTheDocument();
  await user.click(screen.getByText('时间管理', { selector: 'summary' }));
  await user.type(screen.getByRole('textbox', { name: '方向名' }), '训练');
  expect(
    screen.queryByRole('button', { name: '确认并发布方向' }),
  ).not.toBeInTheDocument();
  expect(
    vi
      .mocked(requestJson)
      .mock.calls.filter(([p]) => p.endsWith('/directions')),
  ).toHaveLength(0);
});

it('adopts a generated direction with its frozen evidence and saves it as a draft', async () => {
  const draft = {
    directions: [
      {
        directionRef: 'old',
        label: '时间管理',
        definition: '日程安排',
        topicRefs: ['t'],
      },
    ],
    benchmarkAuthors: [],
  };
  vi.mocked(requestJson).mockImplementation(async (path) =>
    path.includes('/runs')
      ? {
          runs: [
            {
              runRef: 'r',
              state: 'succeeded',
              candidates: [
                {
                  directionRef: 'new',
                  existingDirectionRef: null,
                  label: '在家开始作业',
                  definition: '家长如何协助启动作业',
                  topicRefs: ['t'],
                  topicDefinitionRefs: { t: 'v' },
                  evidenceWorkRefs: ['w'],
                },
              ],
            },
          ],
        }
      : {
          revision: 1,
          publishedRevision: null,
          totalWorks: 1,
          topics: [
            {
              id: 't',
              label: '时间管理',
              definition: '日程安排',
              definitionRef: 'v',
              samples: [],
              parent: null,
            },
          ],
          authors: [],
          draft,
          published: null,
        },
  );
  const user = userEvent.setup();
  render(<OperatingDirections domain="adhd" />);
  await user.click(screen.getByRole('button', { name: '查看运营方向' }));
  await user.click(await screen.findByText('AI 方向初稿与增量建议'));
  await user.click(
    await screen.findByRole('button', { name: '采用候选并调整主题归属' }),
  );
  await user.click(screen.getByRole('button', { name: '保存草案' }));
  expect(
    vi
      .mocked(requestJson)
      .mock.calls.find(
        ([path, opts]) =>
          path.endsWith('/directions') && opts?.method === 'POST',
      )?.[1]?.body,
  ).toMatchObject({
    publish: false,
    catalog: {
      directions: [
        {
          directionRef: 'new',
          textSources: [['t', 'v']],
          evidenceWorkRefs: ['w'],
        },
      ],
    },
  });
});
