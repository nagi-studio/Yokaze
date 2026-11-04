import { describe, expect, test } from 'bun:test';
import {
  createLyricAutoFollow,
  isScrollKey,
} from '../src/utils/lyricAutoFollow';

describe('createLyricAutoFollow', () => {
  test('follows line changes while the user is not scrolling', () => {
    const follow = createLyricAutoFollow(3000);
    expect(follow.shouldScroll(0, true)).toBe(true);
    expect(follow.shouldScroll(50, false)).toBe(false);
  });

  test('manual input pauses following until it has been quiet for the window', () => {
    const follow = createLyricAutoFollow(3000);
    follow.noteManualInput(1000);
    expect(follow.shouldScroll(1500, true)).toBe(false);
    // Trackpad momentum keeps sending wheel events, extending the pause.
    follow.noteManualInput(3500);
    expect(follow.shouldScroll(6000, true)).toBe(false);
    expect(follow.shouldScroll(6499, false)).toBe(false);
  });

  test('scrolls back once when the pause ends, even without a line change', () => {
    const follow = createLyricAutoFollow(3000);
    follow.noteManualInput(0);
    expect(follow.shouldScroll(3000, false)).toBe(true);
    expect(follow.shouldScroll(3050, false)).toBe(false);
    expect(follow.shouldScroll(3100, true)).toBe(true);
  });

  test('reset resumes following immediately without a catch-up scroll', () => {
    const follow = createLyricAutoFollow(3000);
    follow.noteManualInput(0);
    follow.reset();
    expect(follow.shouldScroll(10, false)).toBe(false);
    expect(follow.shouldScroll(20, true)).toBe(true);
  });
});

test('isScrollKey only accepts keys the browser scrolls with', () => {
  expect(['ArrowUp', 'PageDown', 'End', ' '].every(isScrollKey)).toBe(true);
  expect(['ArrowLeft', 'a', 'Enter'].some(isScrollKey)).toBe(false);
});
