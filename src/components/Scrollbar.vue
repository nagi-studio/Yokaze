<template>
  <div>
    <transition name="fade">
      <div
        v-show="show"
        id="scrollbar"
        :class="{ 'on-drag': isOnDrag }"
        @click="handleClick"
      >
        <div
          id="thumbContainer"
          :class="{ active }"
          :style="thumbStyle"
          @mouseenter="handleMouseenter"
          @mouseleave="handleMouseleave"
          @mousedown="handleDragStart"
          @click.stop
        >
          <div></div>
        </div>
      </div>
    </transition>
  </div>
</template>

<script lang="ts">
import { defineComponent, markRaw } from 'vue';
import type { RouteParamsGeneric, RouteRecordName } from 'vue-router';
import {
  computeScrollbarThumb,
  createFrameScheduler,
  thumbDragScrollDelta,
} from '@/utils/scrollFrame';
import type { FrameScheduler, ScrollMetrics } from '@/utils/scrollFrame';

interface StoredPosition {
  scrollTop: number;
  params: RouteParamsGeneric;
}

// Plain, non-reactive per-instance state: none of it drives rendering.
interface ScrollSync {
  main: HTMLElement | null;
  frame: FrameScheduler | null;
  resizeObserver: ResizeObserver | null;
  mutationObserver: MutationObserver | null;
  metrics: ScrollMetrics | null;
  scrolled: boolean;
}

export default defineComponent({
  name: 'Scrollbar',
  emits: {
    'drag-state-change': (dragging: boolean) => typeof dragging === 'boolean',
  },
  data() {
    return {
      top: 0,
      thumbHeight: 0,
      active: false,
      show: false,
      hideTimer: null as ReturnType<typeof setTimeout> | null,
      isOnDrag: false,
      onDragClientY: 0,
      positions: new Map<RouteRecordName, StoredPosition>(),
      sync: markRaw({
        main: null,
        frame: null,
        resizeObserver: null,
        mutationObserver: null,
        metrics: null,
        scrolled: false,
      }) as ScrollSync,
    };
  },
  computed: {
    thumbStyle() {
      return {
        transform: `translateY(${this.top}px)`,
        height: `${this.thumbHeight}px`,
      };
    },
  },

  created() {
    this.sync.frame = createFrameScheduler(() => this.applyFrame());
    this.$router.beforeEach((to, from, next) => {
      // Settle a pending frame against the route it was scrolled on.
      this.sync.frame?.flush();
      this.show = false;
      next();
    });
  },
  mounted() {
    this.attachMain();
  },
  beforeUnmount() {
    this.sync.frame?.cancel();
    this.sync.resizeObserver?.disconnect();
    this.sync.mutationObserver?.disconnect();
    this.sync.resizeObserver = null;
    this.sync.mutationObserver = null;
    if (this.hideTimer !== null) clearTimeout(this.hideTimer);
    if (this.isOnDrag) this.handleDragEnd();
  },

  methods: {
    getMain(): HTMLElement | null {
      if (!this.sync.main) this.attachMain();
      return this.sync.main;
    },
    attachMain() {
      const main = document.querySelector<HTMLElement>(
        'main[data-scroll-container]'
      );
      if (!main || main === this.sync.main) return;
      this.sync.main = main;
      this.sync.metrics = null;
      if (typeof ResizeObserver === 'undefined') return;
      // Viewport resizes show up on main itself; content growth (lazy lists,
      // images) on its page roots, which change as routes swap.
      const resizeObserver = new ResizeObserver(() => this.invalidateMetrics());
      const observeChildren = () => {
        resizeObserver.disconnect();
        resizeObserver.observe(main);
        for (const child of Array.from(main.children)) {
          resizeObserver.observe(child);
        }
      };
      observeChildren();
      const mutationObserver = new MutationObserver(() => {
        observeChildren();
        this.invalidateMetrics();
      });
      mutationObserver.observe(main, { childList: true });
      this.sync.resizeObserver = resizeObserver;
      this.sync.mutationObserver = mutationObserver;
    },
    invalidateMetrics() {
      this.sync.metrics = null;
      this.sync.frame?.schedule();
    },
    readMetrics(main: HTMLElement): ScrollMetrics {
      // Without ResizeObserver nothing would invalidate the cache.
      if (this.sync.metrics && this.sync.resizeObserver) {
        return this.sync.metrics;
      }
      const metrics = {
        clientHeight: main.clientHeight,
        scrollHeight: main.scrollHeight,
      };
      this.sync.metrics = metrics;
      return metrics;
    },
    handleScroll() {
      this.sync.scrolled = true;
      this.sync.frame?.schedule();
    },
    applyFrame() {
      const main = this.getMain();
      if (!main) return;
      const scrolled = this.sync.scrolled;
      this.sync.scrolled = false;
      const scrollTop = main.scrollTop;
      const thumb = computeScrollbarThumb(scrollTop, this.readMetrics(main));
      this.top = thumb.top;
      this.thumbHeight = thumb.height;
      // Size-only updates keep the thumb accurate without revealing it.
      if (!scrolled) return;

      if (!this.show && thumb.scrollable) this.show = true;
      this.setScrollbarHideTimeout();

      const route = this.$route;
      if (route.meta['savePosition'] && route.name !== undefined) {
        this.positions.set(route.name, { scrollTop, params: route.params });
      }
    },
    handleMouseenter() {
      this.active = true;
    },
    handleMouseleave() {
      this.active = false;
      this.setScrollbarHideTimeout();
    },
    handleDragStart(e: MouseEvent) {
      this.onDragClientY = e.clientY;
      this.isOnDrag = true;
      this.$emit('drag-state-change', true);
      document.addEventListener('mousemove', this.handleDragMove);
      document.addEventListener('mouseup', this.handleDragEnd);
    },
    handleDragMove(e: MouseEvent) {
      const main = this.getMain();
      if (!this.isOnDrag || !main) return;
      const clientY = e.clientY;
      const offset = thumbDragScrollDelta(
        clientY - this.onDragClientY,
        this.readMetrics(main)
      );
      // The resulting scroll event moves the thumb on the next frame.
      main.scrollBy(0, offset);
      this.onDragClientY = clientY;
    },
    handleDragEnd() {
      this.isOnDrag = false;
      this.$emit('drag-state-change', false);
      document.removeEventListener('mousemove', this.handleDragMove);
      document.removeEventListener('mouseup', this.handleDragEnd);
    },
    handleClick(e: MouseEvent) {
      const main = this.getMain();
      if (!main) return;
      let scrollTop;
      if (e.clientY < this.top + 84) {
        scrollTop = -256;
      } else {
        scrollTop = 256;
      }
      main.scrollBy({
        top: scrollTop,
        behavior: 'smooth',
      });
    },
    setScrollbarHideTimeout() {
      if (this.hideTimer !== null) clearTimeout(this.hideTimer);
      this.hideTimer = setTimeout(() => {
        if (!this.active) this.show = false;
        this.hideTimer = null;
      }, 4000);
    },
    restorePosition() {
      const route = this.$route;
      const saved =
        route.name === undefined ? undefined : this.positions.get(route.name);
      const main = this.getMain();
      if (!route.meta['savePosition'] || saved === undefined || main === null) {
        return;
      }
      main.scrollTo({ top: saved.scrollTop });
    },
  },
});
</script>

<style lang="scss" scoped>
#scrollbar {
  position: fixed;
  right: 0;
  top: 0;
  bottom: 0;
  width: 16px;
  z-index: 1000;

  #thumbContainer {
    margin-top: 64px;
    div {
      transition: background 0.4s;
      position: absolute;
      right: 2px;
      width: 8px;
      height: 100%;
      border-radius: 4px;
      background: rgba(128, 128, 128, 0.38);
    }
  }
  #thumbContainer.active div {
    background: rgba(128, 128, 128, 0.58);
  }
}

[data-theme='dark'] {
  #thumbContainer div {
    background: var(--color-secondary-bg);
  }
}

#scrollbar.on-drag {
  left: 0;
  width: auto;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
