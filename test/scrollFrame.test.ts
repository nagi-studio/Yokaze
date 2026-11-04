import { describe, expect, test } from 'bun:test';
import {
  computeScrollbarThumb,
  createFrameScheduler,
  thumbDragScrollDelta,
} from '../src/utils/scrollFrame';

function fakeFrames() {
  const queued = new Map<number, FrameRequestCallback>();
  let next = 1;
  return {
    request: (callback: FrameRequestCallback) => {
      const handle = next++;
      queued.set(handle, callback);
      return handle;
    },
    cancel: (handle: number) => {
      queued.delete(handle);
    },
    tick() {
      const callbacks = [...queued.values()];
      queued.clear();
      for (const callback of callbacks) callback(0);
    },
    get size() {
      return queued.size;
    },
  };
}

describe('createFrameScheduler', () => {
  test('merges a burst of scroll events into one frame of work', () => {
    const frames = fakeFrames();
    let runs = 0;
    const scheduler = createFrameScheduler(
      () => runs++,
      frames.request,
      frames.cancel
    );
    for (let i = 0; i < 10; i++) scheduler.schedule();
    expect(frames.size).toBe(1);
    frames.tick();
    expect(runs).toBe(1);
    scheduler.schedule();
    frames.tick();
    expect(runs).toBe(2);
  });

  test('flush runs pending work immediately and only once', () => {
    const frames = fakeFrames();
    let runs = 0;
    const scheduler = createFrameScheduler(
      () => runs++,
      frames.request,
      frames.cancel
    );
    scheduler.flush();
    expect(runs).toBe(0);
    scheduler.schedule();
    scheduler.flush();
    expect(runs).toBe(1);
    frames.tick();
    expect(runs).toBe(1);
  });

  test('cancel drops pending work so unmount leaves no frame behind', () => {
    const frames = fakeFrames();
    let runs = 0;
    const scheduler = createFrameScheduler(
      () => runs++,
      frames.request,
      frames.cancel
    );
    scheduler.schedule();
    scheduler.cancel();
    expect(frames.size).toBe(0);
    frames.tick();
    expect(runs).toBe(0);
    scheduler.schedule();
    expect(frames.size).toBe(1);
  });
});

describe('computeScrollbarThumb', () => {
  const metrics = { clientHeight: 728, scrollHeight: 3128 };

  test('keeps sub-pixel positions so the thumb glides instead of stepping', () => {
    const a = computeScrollbarThumb(10, metrics);
    const b = computeScrollbarThumb(11, metrics);
    expect(b.top - a.top).toBeCloseTo(0.2, 5);
    expect(a.height).toBeCloseTo(120, 5);
    expect(a.scrollable).toBe(true);
  });

  test('pins the thumb to the track ends, including overscroll', () => {
    expect(computeScrollbarThumb(-40, metrics).top).toBe(0);
    const bottom = computeScrollbarThumb(1e6, metrics);
    expect(bottom.top + bottom.height).toBeCloseTo(600, 5);
  });

  test('applies a minimum thumb on very long pages', () => {
    const thumb = computeScrollbarThumb(0, {
      clientHeight: 728,
      scrollHeight: 100_128,
    });
    expect(thumb.height).toBe(24);
  });

  test('reports content that fits as not scrollable', () => {
    expect(
      computeScrollbarThumb(0, { clientHeight: 728, scrollHeight: 728 })
        .scrollable
    ).toBe(false);
    expect(
      computeScrollbarThumb(0, { clientHeight: 100, scrollHeight: 100 })
    ).toEqual({ top: 0, height: 0, scrollable: false });
  });

  test('dragging the thumb by its track length scrolls the whole range', () => {
    expect(thumbDragScrollDelta(600, metrics)).toBeCloseTo(3000, 5);
    expect(thumbDragScrollDelta(-6, metrics)).toBeCloseTo(-30, 5);
  });
});
