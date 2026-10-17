import { describe, expect, it } from 'vitest';
import { parseWorkDetail } from './evidenceDecode';
import { detailJson, mediaHandle, slotJson, workRef } from './evidenceFixtures';
import { mediaObjects, positionText } from './mediaObjects';
import { clampPan, clampZoom, zoomAbout, ZOOM_MAX } from './zoomMath';

const ref = workRef(1);
function inspectorWith(slots: ReturnType<typeof slotJson>[], derivatives: unknown[] = []) {
  return parseWorkDetail(detailJson(1, { slots, derivatives })).inspector;
}

describe('本机媒体对象（J01）', () => {
  it('按生产方采集序号排序，不声称平台排列；作者头像不进轨道，进台账', () => {
    const inspector = inspectorWith([
      slotJson(3, ref, { ordinal: 3 }),
      slotJson(9, ref, { purpose: 'author_avatar', ordinal: 1, slotKey: 'xhs:author:a:avatar:1' }),
      slotJson(1, ref, { ordinal: 1 }),
      slotJson(2, ref, { ordinal: 2 }),
    ]);
    expect(mediaObjects(inspector, null).map(object => object.ordinal)).toEqual([1, 2, 3]);
    expect(mediaObjects(inspector, null, true)).toHaveLength(4);
    expect(positionText(0, 12)).toBe('01 / 12');
  });

  it('只有服务器标记为可内联的图片才给地址；有句柄但不可内联要说明；视频不当图片', () => {
    const objects = mediaObjects(
      inspectorWith([
        slotJson(1, ref, { ordinal: 1 }),
        slotJson(2, ref, { ordinal: 2, deliveryState: 'UNSAFE_MIME', mime: 'image/svg+xml' }),
        slotJson(3, ref, { ordinal: 3, purpose: 'video', mime: 'video/mp4' }),
        slotJson(4, ref, { ordinal: 4, handle: null, bytesState: 'NOT_OBSERVED' }),
      ]),
      null,
    );
    expect(objects[0]).toMatchObject({ assetUrl: mediaHandle(1), inlineBlocked: false });
    expect(objects[1]).toMatchObject({ assetUrl: null, inlineBlocked: true, mime: 'image/svg+xml' });
    expect(objects[2]).toMatchObject({ assetUrl: null, inlineBlocked: false, mime: 'video/mp4' });
    expect(objects[3]).toMatchObject({ assetUrl: null, inlineBlocked: false, bytesState: 'NOT_OBSERVED' });
  });

  it('文字处理状态来自派生物（图片文字、视频画面文字、转录）；原声所在槽位被标出', () => {
    const key = `xhs:${ref}:image:1`;
    const objects = mediaObjects(
      inspectorWith(
        [slotJson(1, ref, { ordinal: 1 }), slotJson(2, ref, { ordinal: 2 })],
        [
          { kind: 'ocr_text', state: 'ACQUIRED', displayText: '第一张图上的字', slotKey: key },
          { kind: 'thumbnail', state: 'QUEUED', slotKey: `xhs:${ref}:image:2` },
        ],
      ),
      { slotKey: key },
    );
    expect(objects[0]).toMatchObject({ derivedState: 'ACQUIRED', derivedText: '第一张图上的字', isMatch: true });
    expect(objects[1]).toMatchObject({ derivedState: null, derivedText: null, isMatch: false });
  });
});

describe('缩放与平移（J04）', () => {
  const frame = { width: 400, height: 300 };
  const image = { width: 400, height: 300 };

  it('缩放被夹在 1×–6×', () => {
    expect(clampZoom(0.2)).toBe(1);
    expect(clampZoom(99)).toBe(ZOOM_MAX);
    expect(clampZoom(2.5)).toBe(2.5);
  });

  it('平移被夹在真实溢出范围内：1× 时不能平移；放大后最多拖到边缘', () => {
    expect(clampPan({ x: -50, y: 30 }, 1, frame, image)).toEqual({ x: 0, y: 0 });
    expect(clampPan({ x: -999, y: 999 }, 2, frame, image)).toEqual({ x: -400, y: 0 });
    expect(clampPan({ x: -10, y: -20 }, 2, frame, image)).toEqual({ x: -10, y: -20 });
  });

  it('以指针为锚缩放：锚点下的像素不动；回到 1× 偏移归零', () => {
    const anchor = { x: 100, y: 50 };
    const out = zoomAbout(1, { x: 0, y: 0 }, 2, anchor);
    expect(out.zoom).toBe(2);
    // 图像坐标 (100,50) 在缩放前后都应落在屏幕 (100,50)：pan + zoom * p = anchor
    expect(out.pan.x + out.zoom * 100).toBeCloseTo(100 * 2 + out.pan.x);
    expect(out.pan).toEqual({ x: -100, y: -50 });
    expect(zoomAbout(2, out.pan, 1, anchor)).toEqual({ zoom: 1, pan: { x: 0, y: 0 } });
    expect(zoomAbout(2, out.pan, 2, anchor)).toEqual({ zoom: 2, pan: out.pan });
    expect(zoomAbout(1, { x: 0, y: 0 }, 3, null).pan).toEqual({ x: 0, y: 0 });
  });
});
