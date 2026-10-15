import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MaterialBrowser } from './MaterialBrowser';
import * as api from './workbenchApi';
import { newDocument, type SourceItem } from './workbenchModel';
vi.mock('./workbenchApi');
const id = '00000000-0000-4000-8000-000000000001';
const item: SourceItem = {
  fragmentId: `${id}.body`,
  sourceKind: 'note',
  sourceRef: id,
  workRef: id,
  field: 'body',
  sourceVersion: 'version1',
  title: '合成原笔记',
  text: '第一段😀\n\n第二段，跨句。保留空白。\n第三段🙂\n第四段',
  bodyState: 'KNOWN',
  authorLabel: '来源材料',
  qualification: 'readable',
  location: null
};
function show(props: Partial<Parameters<typeof MaterialBrowser>[0]> = {}) {
  const client = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false
      }
    }
  });
  return render(<QueryClientProvider client={client}>
<MaterialBrowser domain={id} document={newDocument()} onCollect={vi.fn()} {...props} />
</QueryClientProvider>);
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.sources).mockResolvedValue({
    contract: 'content-workbench-sources.v1',
    asOf: 'now',
    items: [item],
    distribution: [{
      sourceKind: 'note',
      count: 1,
      basis: 'source_directory'
    }],
    total: 1,
    totalBasis: 'source_directory',
    returnedSourceCount: 1,
    returnedFragmentCount: 1,
    nextCursor: null,
    searchScope: 'admitted_native_title_body_history_directory',
    scope: 'collected_material_only'
  });
  vi.mocked(api.source).mockResolvedValue({
    item,
    context: {
      dossier: {
        coverage: {
          complete: false
        }
      }
    }
  });
  vi.mocked(api.errorText).mockReturnValue('来源已受限');
});
describe('V3 exact source reading', () => {
  it('links one fragment identity across strip/index/book and flips full natural paragraphs', async () => {
    show();
    const wall = screen.getByRole('region', {
      name: '分组摘录纸条'
    });
    fireEvent.click(await within(wall).findByRole('button', {
      name: '第一段😀'
    }));
    const leaf = await screen.findByLabelText('可选择原文第 1 段');
    expect(leaf).toHaveTextContent('第一段😀');
    expect(leaf.getAttribute('data-fragment-id')).toBe(within(wall).getByRole('button', {
      name: '第一段😀'
    }).getAttribute('data-fragment-id'));
    expect(screen.getByLabelText('可选择原文第 2 段')).toHaveTextContent('第二段，跨句。保留空白。');
    fireEvent.click(screen.getByRole('button', {
      name: '下一展开页 →'
    }));
    expect(await screen.findByLabelText('可选择原文第 3 段')).toHaveTextContent('第三段🙂');
    expect(screen.getByLabelText('可选择原文第 4 段')).toHaveTextContent('第四段');
  });
  it('never reuses list raw text in the book when current source reading is restricted', async () => {
    vi.mocked(api.source).mockRejectedValue(new Error('restricted'));
    show();
    const wall = screen.getByRole('region', {
      name: '分组摘录纸条'
    });
    fireEvent.click(await within(wall).findByRole('button', {
      name: '第一段😀'
    }));
    await waitFor(() => expect(api.source).toHaveBeenCalled());
    expect(screen.queryByLabelText('可选择原文第 1 段')).not.toBeInTheDocument();
    expect(screen.getByRole('button', {
      name: '摘入画布'
    })).toBeDisabled();
  });
  it('clears book/selection when usage filter excludes the active strip', async () => {
    show();
    const wall = screen.getByRole('region', {
      name: '分组摘录纸条'
    });
    fireEvent.click(await within(wall).findByRole('button', {
      name: '第一段😀'
    }));
    await screen.findByLabelText('可选择原文第 1 段');
    fireEvent.click(screen.getByRole('button', {
      name: '已摘录'
    }));
    expect(screen.queryByLabelText('可选择原文第 1 段')).not.toBeInTheDocument();
    expect(screen.getByRole('button', {
      name: '摘入画布'
    })).toBeDisabled();
  });
  it('opens a full citation even if its source is not on the directory page', async () => {
    vi.mocked(api.sources).mockResolvedValue({
      contract: 'content-workbench-sources.v1',
      asOf: 'now',
      items: [],
      distribution: [],
      total: 0,
      totalBasis: 'source_directory',
      returnedSourceCount: 0,
      returnedFragmentCount: 0,
      nextCursor: null,
      searchScope: 'admitted_native_title_body_history_directory',
      scope: 'collected_material_only'
    });
    show({
      focusCitation: {
        sourceKind: 'note',
        sourceRef: id,
        workRef: id,
        field: 'body',
        sourceVersion: 'version1',
        start: 0,
        end: 4,
        textHash: 'h',
        location: null
      }
    });
    expect(await screen.findByLabelText('可选择原文第 1 段')).toHaveTextContent('第一段😀');
    expect(screen.getByRole('button', {
      name: '摘入画布'
    })).toBeEnabled();
  });
});
it('marks a partial emoji excerpt as collected and includes every overlapping group without claiming unrelated paragraphs', async () => {
  const document = newDocument();
  const first = crypto.randomUUID(),
    second = crypto.randomUUID();
  document.excerpts = [first, second].map((excerptId, i) => ({
    id: excerptId,
    citation: {
      sourceKind: 'note',
      sourceRef: id,
      workRef: id,
      field: 'body',
      sourceVersion: item.sourceVersion,
      start: i === 0 ? 1 : 2,
      end: 4,
      textHash: 'h',
      location: null
    },
    selectedText: i === 0 ? '一段😀' : '段😀',
    textHash: 'h'
  }));
  document.canvas = [{
    id: crypto.randomUUID(),
    excerptId: first,
    group: 'argument',
    note: '',
    expression: '',
    expressionRevision: 1
  }, {
    id: crypto.randomUUID(),
    excerptId: second,
    group: 'opening',
    note: '',
    expression: '',
    expressionRevision: 1
  }];
  show({
    document
  });
  const wall = screen.getByRole('region', {
    name: '分组摘录纸条'
  });
  expect(await within(wall).findByRole('button', {
    name: '第一段😀'
  })).toHaveAttribute('data-collected', 'true');
  fireEvent.click(screen.getByRole('button', {
    name: '已摘录'
  }));
  expect(within(wall).getByRole('button', {
    name: '第一段😀'
  })).toBeInTheDocument();
  expect(within(wall).queryByRole('button', {
    name: '第三段🙂'
  })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', {
    name: '未摘录'
  }));
  expect(within(wall).queryByRole('button', {
    name: '第一段😀'
  })).not.toBeInTheDocument();
  expect(within(wall).getByRole('button', {
    name: '第三段🙂'
  })).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', {
    name: '开头'
  }));
  expect(within(wall).getByRole('button', {
    name: '第一段😀'
  })).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', {
    name: '论点'
  }));
  expect(within(wall).getByRole('button', {
    name: '第一段😀'
  })).toBeInTheDocument();
});
