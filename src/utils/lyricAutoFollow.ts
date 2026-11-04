export const LYRIC_FOLLOW_PAUSE_MS = 3000;

export interface LyricAutoFollow {
  /** Record a user scroll gesture; auto-follow pauses until it goes quiet. */
  noteManualInput(now: number): void;
  /**
   * Whether the view should scroll to the active line on this tick. True when
   * the line changed while following, and once when a pause runs out so the
   * view returns to the current line.
   */
  shouldScroll(now: number, lineChanged: boolean): boolean;
  reset(): void;
}

// Only real input events pause following; the view's own smooth scrolls fire
// `scroll` but never wheel/touch/key, so they cannot be mistaken for the user.
export function createLyricAutoFollow(
  pauseMs: number = LYRIC_FOLLOW_PAUSE_MS
): LyricAutoFollow {
  let pausedUntil: number | null = null;
  return {
    noteManualInput(now) {
      pausedUntil = now + pauseMs;
    },
    shouldScroll(now, lineChanged) {
      if (pausedUntil === null) return lineChanged;
      if (now < pausedUntil) return false;
      pausedUntil = null;
      return true;
    },
    reset() {
      pausedUntil = null;
    },
  };
}

const SCROLL_KEYS = new Set([
  'ArrowUp',
  'ArrowDown',
  'PageUp',
  'PageDown',
  'Home',
  'End',
  ' ',
]);

export function isScrollKey(key: string): boolean {
  return SCROLL_KEYS.has(key);
}
