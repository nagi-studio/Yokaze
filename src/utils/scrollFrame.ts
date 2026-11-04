export interface FrameScheduler {
  /** Queue `run` for the next frame; repeated calls before it fires merge. */
  schedule(): void;
  /** Run a queued frame now (e.g. before the route changes under it). */
  flush(): void;
  cancel(): void;
}

type RequestFrame = (callback: FrameRequestCallback) => number;
type CancelFrame = (handle: number) => void;

// Scroll events can fire several times per frame; one frame of work is enough.
export function createFrameScheduler(
  run: () => void,
  requestFrame: RequestFrame = callback => requestAnimationFrame(callback),
  cancelFrame: CancelFrame = handle => cancelAnimationFrame(handle)
): FrameScheduler {
  let handle: number | null = null;
  const fire = () => {
    handle = null;
    run();
  };
  return {
    schedule() {
      if (handle === null) handle = requestFrame(fire);
    },
    flush() {
      if (handle === null) return;
      cancelFrame(handle);
      fire();
    },
    cancel() {
      if (handle === null) return;
      cancelFrame(handle);
      handle = null;
    },
  };
}

export interface ScrollMetrics {
  clientHeight: number;
  scrollHeight: number;
}

export interface ScrollbarThumb {
  top: number;
  height: number;
  /** False when the content fits, so the thumb would fill the whole track. */
  scrollable: boolean;
}

// Navbar + player cover 128px of the scroll container; the track excludes them.
export const SCROLLBAR_INSET = 128;
export const SCROLLBAR_MIN_THUMB = 24;

// Fractional output on purpose: rounding made the thumb step while scrolling.
export function computeScrollbarThumb(
  scrollTop: number,
  { clientHeight, scrollHeight }: ScrollMetrics
): ScrollbarThumb {
  const track = clientHeight - SCROLLBAR_INSET;
  const range = scrollHeight - SCROLLBAR_INSET;
  if (track <= 0 || range <= 0) return { top: 0, height: 0, scrollable: false };
  const height = Math.max(SCROLLBAR_MIN_THUMB, (track / range) * track);
  const top = Math.min(
    Math.max(0, (scrollTop / range) * track),
    Math.max(0, track - height)
  );
  return { top, height, scrollable: height < track };
}

/** Converts a pointer delta along the track into a scroll delta. */
export function thumbDragScrollDelta(
  pointerDelta: number,
  { clientHeight, scrollHeight }: ScrollMetrics
): number {
  const track = clientHeight - SCROLLBAR_INSET;
  if (track <= 0) return 0;
  return (pointerDelta / track) * (scrollHeight - SCROLLBAR_INSET);
}
