import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { newImagePage } from '../imagePageFactory';
import * as renderer from '../renderer';
import { CoverCanvas } from './CoverCanvas';

vi.mock('../renderer');

const context = { drawImage: vi.fn() };
beforeEach(() => {
  vi.resetAllMocks();
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(context as unknown as CanvasRenderingContext2D);
});

describe('cover preview', () => {
  it('draws the cover through the export renderer, with the uploaded image path when the page has one', async () => {
    const drawn = document.createElement('canvas'); drawn.width = 540; drawn.height = 720;
    vi.mocked(renderer.renderPage).mockResolvedValue({ canvas: drawn, issues: [], fontFamily: 'x' });
    const page = { ...newImagePage('cover'), title: '先做一步', assetId: '00000000-0000-4000-8000-0000000000a5' };
    render(<CoverCanvas page={page} workId="w1" />);
    await waitFor(() => expect(context.drawImage).toHaveBeenCalledWith(drawn, 0, 0));
    expect(renderer.renderPage).toHaveBeenCalledWith(page, expect.objectContaining({ assetUrl: '/ops/api/creation/works/w1/assets/00000000-0000-4000-8000-0000000000a5' }));
    expect(screen.getByLabelText('封面预览：先做一步')).toBeInTheDocument();
    expect(screen.queryByRole('status')).toBeNull();
  });
  it('says the preview is unavailable when the renderer fails, instead of leaving a blank frame', async () => {
    vi.mocked(renderer.renderPage).mockRejectedValue(new Error('字体暂时不可用'));
    render(<CoverCanvas page={newImagePage('cover')} workId="w1" />);
    expect(await screen.findByRole('status')).toHaveTextContent('封面预览暂时不可用：字体暂时不可用');
    expect(screen.getByLabelText('封面预览：尚未填写')).toBeInTheDocument();
  });
});
