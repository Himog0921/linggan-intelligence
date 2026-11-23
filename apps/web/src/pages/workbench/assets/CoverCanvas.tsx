import { useEffect, useRef, useState } from 'react';
import type { ImagePage } from '../workbenchModel';
import { renderPage } from '../renderer';

/** The cover drawn by the same renderer that exports it, so the preview is the page that would be exported. */
export function CoverCanvas({ page, workId, quiet = false }: { page: ImagePage; workId: string; /** Draw only the canvas, with no failure line (for a place that cannot hold a paragraph, such as inside a button). */ quiet?: boolean }) {
  const host = useRef<HTMLCanvasElement>(null);
  const [failure, setFailure] = useState<{ page: ImagePage; message: string } | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    void renderPage(page, { signal: controller.signal, assetUrl: page.assetId ? `/ops/api/creation/works/${encodeURIComponent(workId)}/assets/${encodeURIComponent(page.assetId)}` : undefined })
      .then(result => {
        if (controller.signal.aborted || !host.current) return;
        host.current.width = result.canvas.width;
        host.current.height = result.canvas.height;
        host.current.getContext('2d')?.drawImage(result.canvas, 0, 0);
        setFailure(null);
      })
      .catch((cause: unknown) => {
        if (!controller.signal.aborted) setFailure({ page, message: cause instanceof Error ? cause.message : '请重新打开或替换图片' });
      });
    return () => controller.abort();
  }, [page, workId]);
  return <>
    <canvas ref={host} width={1080} height={1440} style={{ display: 'block', width: '100%', height: '100%' }} aria-label={`封面预览：${page.title || '尚未填写'}`} />
    {!quiet && failure && failure.page === page ? <p className="aux" role="status">封面预览暂时不可用：{failure.message}</p> : null}
  </>;
}
