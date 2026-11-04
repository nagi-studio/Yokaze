<template>
  <div
    id="app"
    :class="{
      'user-select-none': userSelectNone,
      'window-hidden': windowHidden,
      'compact-curtain': compactCurtain,
    }"
    :data-compact-expansion="compactBrowseStage"
    :data-compact-motion="compactBrowseStage && compactExpansion.motion"
    :style="compactExpansionStyle"
    @animationend="handleCompactEnterEnd"
  >
    <Scrollbar
      v-show="!showLyrics"
      ref="scrollbar"
      @drag-state-change="userSelectNone = $event"
    />
    <Navbar
      v-show="showNavbar"
      ref="navbar"
      :compact-window-expanded="compactWindowExpanded"
      @restore-compact-window="restoreCompactWindow"
    />
    <main
      ref="main"
      data-scroll-container
      :style="{ overflow: enableScrolling ? 'auto' : 'hidden' }"
      @scroll="handleScroll"
    >
      <router-view v-slot="{ Component, route }">
        <keep-alive :max="4">
          <component
            :is="Component"
            v-if="route.meta['keepAlive']"
            :key="route.fullPath"
          />
        </keep-alive>
        <component
          :is="Component"
          v-if="!route.meta['keepAlive']"
          :key="route.fullPath"
        />
      </router-view>
    </main>
    <transition name="slide-up">
      <Player v-if="enablePlayer" v-show="showPlayer" ref="player" />
    </transition>
    <Toast />
    <ModalAddTrackToPlaylist v-if="isAccountLoggedIn" />
    <ModalNewPlaylist v-if="isAccountLoggedIn" />
    <ModalCloseApp
      :show="closePromptVisible"
      @cancel="cancelCloseChoice"
      @choose="resolveCloseChoice"
    />
    <transition
      v-if="enablePlayer"
      :name="lyricsTransitionName"
      @after-leave="handleLyricsAfterLeave"
      @after-enter="dispatchCompactExpansion({ type: 'barShown' })"
    >
      <Lyrics
        v-show="showLyrics"
        @expand-compact-window="expandCompactWindow"
      />
    </transition>
  </div>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
import ModalAddTrackToPlaylist from './components/ModalAddTrackToPlaylist.vue';
import ModalNewPlaylist from './components/ModalNewPlaylist.vue';
import ModalCloseApp from './components/ModalCloseApp.vue';
import Scrollbar from './components/Scrollbar.vue';
import Navbar from './components/Navbar.vue';
import Player from './components/Player.vue';
import Toast from './components/Toast.vue';
import { isAccountLoggedIn, isLooseLoggedIn } from '@/utils/auth';
import { refreshAccountData } from '@/services/accountBootstrap';
import Lyrics from './views/lyrics.vue';
import { mapActions, mapState } from 'pinia';
import { useAppStore } from '@/stores/app';
import { observeDocumentVisibility } from '@/utils/mediaLifecycle';
import { connectDesktopEvents } from '@/services/desktopBridge';
import { sendDesktop } from '@/services/desktopTransport';
import { isDesktopRuntime } from '@/utils/runtime';
import {
  applyCompactExpansion,
  COMPACT_RESIZE_SETTLE_MS,
  hasRememberedBarFrame,
  planCompactExpansion,
  rememberCurrentCompactWindowFrame,
  restoreRememberedCompactWindowFrame,
  restoreCompactWindow,
  signalInitialWindowReady,
} from '@/services/compactWindow';
import {
  COMPACT_BAR_FADE_MS,
  COMPACT_ENTER_MS,
  compactBrowseStage,
  compactExpansionDeadline,
  IDLE_COMPACT_EXPANSION,
  nextCompactExpansion,
  resolveAutoLyrics,
  resolveSettledLyrics,
  type AutoLyricsAction,
  type CompactExpansionEffect,
  type CompactExpansionEvent,
  type CompactExpansionMotion,
} from '@/utils/compactExpansion';
import {
  isBarWindowSize,
  isMiniWindowSize,
  isWindowsTauri,
} from '@/utils/miniWindow';
import {
  isEditableShortcutTarget,
  resolveRuntimeShortcutAction,
  runLocalShortcutAction,
} from '@/services/localShortcuts';
import { checkForAppUpdateInBackground } from '@/services/appUpdater';
import { isMac } from '@/utils/platform';
import type { AppShell } from '@/types/appShell';
import type { RouteLocationRaw } from 'vue-router';
import { isLastfmCallbackLocation } from '@/services/lastfmAuth';

function nextAnimationFrame(): Promise<void> {
  return new Promise(resolve => requestAnimationFrame(() => resolve()));
}

export default defineComponent({
  name: 'App',
  provide() {
    const appShell: AppShell = {
      restoreScrollPosition: () =>
        (
          this.$refs['scrollbar'] as InstanceType<typeof Scrollbar> | undefined
        )?.restorePosition(),
      scrollMainTo: (optionsOrX?: ScrollToOptions | number, y?: number) => {
        const main = this.$refs['main'] as HTMLElement | undefined;
        if (!main) return;
        if (typeof optionsOrX === 'number') main.scrollTo(optionsOrX, y ?? 0);
        else main.scrollTo(optionsOrX);
      },
    };
    return {
      appShell,
    };
  },
  components: {
    Navbar,
    Player,
    Toast,
    ModalAddTrackToPlaylist,
    ModalNewPlaylist,
    ModalCloseApp,
    Lyrics,
    Scrollbar,
  },
  data() {
    return {
      isDesktop: isDesktopRuntime,
      isLastfmCallback: isLastfmCallbackLocation(window.location),
      userSelectNone: false,
      autoOpenedLyrics: false, // Restore lyrics only when compact mode opened it.
      // Set while an expansion closes lyrics: the window is already full size,
      // so sliding the full player away reads as a flash.
      lyricsLeaveInstant: false,
      // Covers the page while an unstaged expansion resizes the window, so
      // neither the squeezed browse view nor a full-size player flashes.
      compactCurtain: false,
      compactExpansion: IDLE_COMPACT_EXPANSION,
      compactExpansionDeadline: null as ReturnType<typeof setTimeout> | null,
      compactWindowExpanded:
        !isBarWindowSize({
          width: window.innerWidth,
          height: window.innerHeight,
        }) && hasRememberedBarFrame(),
      compactResizeTimer: null as ReturnType<typeof setTimeout> | null,
      compactWindowMemoryReady: !isDesktopRuntime,
      windowHidden: document.hidden,
      visibilityCleanup: null as (() => void) | null,
      desktopEventsCleanup: null as Promise<() => void> | null,
      closePromptVisible: false,
    };
  },
  computed: {
    ...mapState(useAppStore, [
      'showLyrics',
      'settings',
      'player',
      'enableScrolling',
    ]),
    isAccountLoggedIn() {
      return isAccountLoggedIn();
    },
    showPlayer() {
      return (
        [
          'mv',
          'loginUsername',
          'login',
          'loginAccount',
          'lastfmCallback',
        ].includes(String(this.$route.name ?? '')) === false
      );
    },
    enablePlayer() {
      return this.player.enabled && this.$route.name !== 'lastfmCallback';
    },
    showNavbar() {
      return this.$route.name !== 'lastfmCallback';
    },
    compactBrowseStage() {
      return compactBrowseStage(this.compactExpansion);
    },
    lyricsTransitionName(): string {
      if (this.compactBrowseStage === 'hidden') return 'compact-bar-fade';
      // No CSS exists for this name, so Vue finishes the leave immediately.
      return this.lyricsLeaveInstant ? 'lyrics-instant' : 'slide-up';
    },
    compactExpansionStyle() {
      if (!this.compactBrowseStage) return undefined;
      const { plan, motion } = this.compactExpansion;
      return {
        '--compact-enter-from': plan?.origin === 'bottom' ? '1' : '-1',
        '--compact-enter-duration': `${COMPACT_ENTER_MS[motion]}ms`,
        '--compact-bar-fade-duration': `${COMPACT_BAR_FADE_MS}ms`,
      };
    },
  },
  watch: {
    '$route.name'(name: unknown) {
      this.dispatchCompactExpansion({ type: 'routeChanged', name });
    },
  },
  created() {
    if (this.isLastfmCallback) return;
    if (this.isDesktop && !this.isLastfmCallback) {
      this.desktopEventsCleanup = connectDesktopEvents({
        pushRoute: path => this.$router.push(path as RouteLocationRaw),
        focusSearch: () =>
          (this.$refs['navbar'] as InstanceType<typeof Navbar>).focusSearch(),
        goHistory: where =>
          (this.$refs['navbar'] as InstanceType<typeof Navbar>).go(where),
        goToNextTracksPage: () =>
          (
            this.$refs['player'] as InstanceType<typeof Player>
          ).goToNextTracksPage(),
        requestCloseChoice: () => (this.closePromptVisible = true),
      });
    }
    window.addEventListener('keydown', this.handleKeydown);
    window.addEventListener('resize', this.handleMiniResize);
    this.visibilityCleanup = observeDocumentVisibility(
      document,
      hidden => (this.windowHidden = hidden)
    );
    if (this.isDesktop && !this.isLastfmCallback) {
      void this.initializeCompactWindowMemory();
    } else {
      this.handleMiniResize();
    }
    if (!this.isLastfmCallback) this.fetchData();
    if (this.isDesktop && !this.isLastfmCallback)
      void this.checkForUpdateOnStartup();
  },
  beforeUnmount() {
    window.removeEventListener('keydown', this.handleKeydown);
    window.removeEventListener('resize', this.handleMiniResize);
    if (this.compactResizeTimer !== null) clearTimeout(this.compactResizeTimer);
    this.clearCompactExpansionDeadline();
    this.visibilityCleanup?.();
    this.desktopEventsCleanup?.then(cleanup => cleanup());
  },
  methods: {
    ...mapActions(useAppStore, [
      'toggleLyrics',
      'fetchLikedSongs',
      'fetchLikedSongsWithDetails',
      'fetchLikedPlaylist',
      'fetchUserProfile',
      'likeATrack',
      'showToast',
    ]),
    async checkForUpdateOnStartup() {
      const result = await checkForAppUpdateInBackground();
      if (result?.status !== 'available') return;
      this.showToast(
        String(
          this.$t('settings.updater.available', {
            version: result.version,
          })
        )
      );
    },
    async initializeCompactWindowMemory() {
      try {
        const restored = await restoreRememberedCompactWindowFrame();
        this.compactWindowExpanded =
          restored?.mode === 'browse' && hasRememberedBarFrame();
      } catch (error) {
        // Keep resizing usable after a one-off restore failure.
        console.warn('[compact-window] startup restore failed', error);
      } finally {
        // Persist only after restoring logical dimensions.
        this.compactWindowMemoryReady = true;
        this.handleMiniResize();
        await this.$nextTick();
        try {
          await signalInitialWindowReady();
        } catch (error) {
          console.warn('[compact-window] readiness signal failed', error);
        }
      }
    },
    async resolveCloseChoice(payload: {
      action: 'exit' | 'minimizeToTray';
      remember: boolean;
    }) {
      this.closePromptVisible = false;
      await sendDesktop('resolveCloseChoice', payload);
    },
    async cancelCloseChoice() {
      this.closePromptVisible = false;
      await sendDesktop('cancelCloseChoice');
    },
    expandCompactWindow() {
      this.dispatchCompactExpansion({
        type: 'expand',
        motion: this.compactExpansionMotion(),
      });
    },
    restoreCompactWindow() {
      this.dispatchCompactExpansion({ type: 'restore' });
    },
    handleLyricsAfterLeave() {
      this.lyricsLeaveInstant = false;
      this.dispatchCompactExpansion({ type: 'barHidden' });
    },
    compactExpansionMotion(): CompactExpansionMotion {
      if (!isWindowsTauri) return 'none';
      const reduced = window.matchMedia(
        '(prefers-reduced-motion: reduce)'
      ).matches;
      return this.settings.windowsWindowAnimations && !reduced
        ? 'full'
        : 'fade';
    },
    dispatchCompactExpansion(event: CompactExpansionEvent) {
      const previous = this.compactExpansion;
      const { state, effects } = nextCompactExpansion(previous, event);
      this.compactExpansion = state;
      // Whatever path the expansion took, it must never end hidden.
      if (state.phase === 'idle') this.compactCurtain = false;
      if (state.phase !== previous.phase) {
        this.clearCompactExpansionDeadline();
        const deadline = compactExpansionDeadline(state);
        if (deadline) {
          this.compactExpansionDeadline = setTimeout(() => {
            this.compactExpansionDeadline = null;
            this.dispatchCompactExpansion(deadline.event);
          }, deadline.ms);
        }
      }
      for (const effect of effects) {
        void this.runCompactExpansionEffect(effect);
      }
    },
    async runCompactExpansionEffect(effect: CompactExpansionEffect) {
      switch (effect) {
        case 'plan': {
          const plan = await planCompactExpansion().catch(error => {
            console.warn('[compact-window] expansion planning failed', error);
            return null;
          });
          this.dispatchCompactExpansion(
            plan ? { type: 'planned', plan } : { type: 'planFailed' }
          );
          return;
        }
        case 'hideBar':
        case 'showBar': {
          const event = effect === 'hideBar' ? 'barHidden' : 'barShown';
          // The lyrics transition reports completion via after-leave/enter.
          if (this.showLyrics === (effect === 'showBar')) {
            this.dispatchCompactExpansion({ type: event });
            return;
          }
          this.toggleLyrics();
          return;
        }
        case 'resize': {
          const { plan, motion } = this.compactExpansion;
          if (motion === 'none' && plan) {
            this.compactCurtain = true;
            if (this.showLyrics) {
              this.lyricsLeaveInstant = true;
              this.toggleLyrics();
            }
            await this.$nextTick();
            await nextAnimationFrame();
          }
          const size = plan
            ? await applyCompactExpansion(plan).catch(error => {
                console.warn('[compact-window] expansion failed', error);
                return null;
              })
            : null;
          if (!size) this.compactCurtain = false;
          this.dispatchCompactExpansion(
            size ? { type: 'resized', size } : { type: 'resizeFailed' }
          );
          return;
        }
        case 'showQueue': {
          // The user asked for the queue, so it replaces the compact lyrics.
          this.compactWindowExpanded = true;
          if (this.showLyrics) {
            this.lyricsLeaveInstant = true;
            this.toggleLyrics();
          }
          this.autoOpenedLyrics = false;
          if (this.$route.name !== 'next') {
            await this.$router.push({ name: 'next' }).catch(error => {
              console.warn('[compact-window] queue navigation failed', error);
            });
          }
          await this.$nextTick();
          if (this.compactCurtain) {
            // Lift only after the queue has laid out at the new size.
            await nextAnimationFrame();
            this.compactCurtain = false;
          }
          this.dispatchCompactExpansion({
            type: 'queueShown',
            onQueue: this.$route.name === 'next',
          });
          return;
        }
        case 'restore':
          try {
            if (await restoreCompactWindow()) {
              this.compactWindowExpanded = false;
            }
          } catch (error) {
            console.warn('[compact-window] restore failed', error);
          }
          this.dispatchCompactExpansion({ type: 'restored' });
          return;
        case 'settle':
          // Reconcile lyrics with the size the expansion left behind. Only a
          // real resize event may end the queue choice (becameBar).
          this.applyAutoLyrics(
            resolveSettledLyrics(
              this.compactExpansion,
              { width: window.innerWidth, height: window.innerHeight },
              { showLyrics: this.showLyrics, autoOpened: this.autoOpenedLyrics }
            )
          );
          return;
      }
    },
    handleCompactEnterEnd(event: AnimationEvent) {
      if (event.target !== this.$refs['main']) return;
      this.dispatchCompactExpansion({ type: 'entered' });
    },
    clearCompactExpansionDeadline() {
      if (this.compactExpansionDeadline === null) return;
      clearTimeout(this.compactExpansionDeadline);
      this.compactExpansionDeadline = null;
    },
    applyAutoLyrics(action: AutoLyricsAction) {
      if (!action) return;
      this.toggleLyrics();
      this.autoOpenedLyrics = action === 'open';
    },
    // Open the lyrics player when the window becomes narrow or short.
    handleMiniResize() {
      const size = {
        width: window.innerWidth,
        height: window.innerHeight,
      };
      const isBar = isBarWindowSize(size);
      // The Esc pill follows the bar/browse split, not the lyrics routing.
      this.compactWindowExpanded = !isBar && hasRememberedBarFrame();
      if (isBar) this.dispatchCompactExpansion({ type: 'becameBar' });
      this.applyAutoLyrics(
        resolveAutoLyrics(this.compactExpansion, {
          isMini: isMiniWindowSize(size),
          showLyrics: this.showLyrics,
          autoOpened: this.autoOpenedLyrics,
        })
      );
      this.scheduleCompactWindowMemory();
    },
    scheduleCompactWindowMemory() {
      if (!this.isDesktop || !this.compactWindowMemoryReady) return;
      if (this.compactResizeTimer !== null)
        clearTimeout(this.compactResizeTimer);
      // Persist only the settled layout mode.
      this.compactResizeTimer = setTimeout(async () => {
        const remembered = await rememberCurrentCompactWindowFrame();
        if (!remembered) return;
        this.compactWindowExpanded =
          remembered.mode === 'browse' && hasRememberedBarFrame();
      }, COMPACT_RESIZE_SETTLE_MS);
    },
    handleKeydown(e: KeyboardEvent) {
      if (e.defaultPrevented) return;
      if (e.code === 'Escape' && this.compactWindowExpanded) {
        e.preventDefault();
        void this.restoreCompactWindow();
        return;
      }
      if (isEditableShortcutTarget(e.target)) return;
      const action = resolveRuntimeShortcutAction(
        this.settings.shortcuts,
        e,
        isMac,
        this.isDesktop
      );
      if (!action) return;

      e.preventDefault();
      if (action === 'play' && this.$route.name === 'mv') return;
      const player = this.player;
      runLocalShortcutAction(action, {
        isPersonalFM: player.isPersonalFM,
        get volume() {
          return player.volume;
        },
        set volume(value: number) {
          player.volume = value;
        },
        currentTrackId: player.currentTrack.id,
        playOrPause: () => player.playOrPause(),
        playNextFMTrack: () => player.playNextFMTrack(),
        playNextTrack: () => player.playNextTrack(),
        playPrevTrack: () => player.playPrevTrack(),
        likeTrack: id => this.likeATrack(id),
        minimize: () => sendDesktop('minimize'),
      });
    },
    async fetchData() {
      if (!isLooseLoggedIn()) return;
      try {
        await refreshAccountData({
          accountLoggedIn: isAccountLoggedIn(),
          fetchUserProfile: () => this.fetchUserProfile(),
          fetchLikedSongs: () => this.fetchLikedSongs(),
          fetchLikedPlaylist: () => this.fetchLikedPlaylist(),
          fetchLikedSongsWithDetails: () => this.fetchLikedSongsWithDetails(),
        });
      } catch (error) {
        console.warn('[account] Unable to refresh account data', error);
      }
      // Load large library collections only when the library opens.
    },
    handleScroll() {
      (
        this.$refs['scrollbar'] as InstanceType<typeof Scrollbar>
      ).handleScroll();
    },
  },
});
</script>

<style lang="scss">
#app {
  width: 100%;
  transition: all 0.4s;
}

// Pause visual effects while the hidden WebView keeps running.
#app.window-hidden *,
#app.window-hidden *::before,
#app.window-hidden *::after {
  animation-play-state: paused !important;
}

main {
  position: fixed;
  top: 0;
  bottom: 0;
  right: 0;
  left: 0;
  overflow: auto;
  padding: 64px 10vw 96px 10vw;
  box-sizing: border-box;
  scrollbar-width: none; // firefox
}

@media (max-width: 1336px) {
  main {
    padding: 64px 5vw 96px 5vw;
  }
}

main::-webkit-scrollbar {
  width: 0px;
}

.slide-up-enter-active,
.slide-up-leave-active {
  transition: transform 0.4s;
}
.slide-up-enter-from,
.slide-up-leave-to {
  transform: translateY(100%);
}

// Windows compact expansion: the browse view stays hidden while the bar fades
// and the window resizes, then enters from the edge that stayed put.
// Invisible or still moving content must not take clicks.
#app[data-compact-expansion] :is(main, nav, .player) {
  pointer-events: none;
}

#app.compact-curtain :is(main, nav, .player) {
  visibility: hidden;
}

#app[data-compact-expansion='hidden'] :is(main, nav, .player) {
  opacity: 0;
}

#app[data-compact-expansion='entering'] :is(main, nav, .player) {
  transform-origin: 50% calc(50% + var(--compact-enter-from) * 50%);
  animation: compact-browse-enter var(--compact-enter-duration)
    cubic-bezier(0.32, 0.72, 0, 1) both;
}

#app[data-compact-expansion='entering'][data-compact-motion='fade']
  :is(main, nav, .player) {
  animation-name: compact-browse-fade;
}

@keyframes compact-browse-enter {
  from {
    opacity: 0;
    transform: translateY(calc(var(--compact-enter-from) * 12px)) scale(0.97);
  }
}

@keyframes compact-browse-fade {
  from {
    opacity: 0;
  }
}

.compact-bar-fade-enter-active,
.compact-bar-fade-leave-active {
  transition: opacity var(--compact-bar-fade-duration) ease-out;
}
.compact-bar-fade-enter-from,
.compact-bar-fade-leave-to {
  opacity: 0;
}
</style>
