<template>
  <transition name="slide-up">
    <div
      data-tauri-drag-region
      class="lyrics-page"
      :class="{
        'no-lyric': noLyric,
        'controls-bottom': settings.lyricsPortraitControls === 'bottom',
      }"
      :data-theme="theme"
    >
      <div
        v-if="
          settings.lyricsBackground === 'blur' ||
          settings.lyricsBackground === 'dynamic'
        "
        data-tauri-drag-region
        class="lyrics-background"
        :class="{
          'dynamic-background': settings.lyricsBackground === 'dynamic',
        }"
      >
        <div
          data-tauri-drag-region
          class="top-right"
          :style="{ backgroundImage: `url(${bgImageUrl})` }"
        />
        <div
          data-tauri-drag-region
          class="bottom-left"
          :style="{ backgroundImage: `url(${bgImageUrl})` }"
        />
      </div>
      <div
        v-if="settings.lyricsBackground === true"
        data-tauri-drag-region
        class="gradient-background"
        :style="{ background }"
      ></div>

      <!-- Compact mode: artwork and one lyric line. -->
      <div
        v-if="isMini"
        class="mini-player"
        :class="{
          'mini-text-locked': miniTextLocked,
          'taskbar-snap-ready': taskbarDockHint === 'ready',
        }"
        @mouseenter="setWindowButtons(true)"
        @mouseleave="handleMiniMouseLeave"
        @mousedown="handleMiniMouseDown"
        @dblclick="handleMiniDoubleClick"
      >
        <img class="mini-cover" :src="miniImageUrl" />
        <!-- Keep blank space draggable while allowing text selection. -->
        <div class="mini-info">
          <div class="mini-title" :title="currentTrack.name">
            <span class="mini-copyable">{{ currentTrack.name }}</span>
          </div>
          <div class="mini-artist" :title="artist.name">
            <span class="mini-copyable">{{ artist.name }}</span>
          </div>
        </div>
        <div
          class="mini-lyric"
          :class="{ placeholder: noLyric }"
          :title="displayLyric"
        >
          <div class="mini-lyric-origin" :style="miniLyricFontSize">
            <span class="mini-copyable">{{ displayLyric }}</span>
          </div>
          <div v-if="showMiniTranslation" class="mini-lyric-translation">
            <span class="mini-copyable">{{ currentLyricTranslation }}</span>
          </div>
        </div>
        <div class="mini-controls">
          <button-icon
            class="mini-pin"
            :class="{
              active: isAlwaysOnTop,
              'pin-dismissed': pinDismissed,
            }"
            :title="isAlwaysOnTop ? '取消置顶' : '窗口置顶'"
            @click="toggleAlwaysOnTop"
          >
            <svg-icon icon-class="pin" />
          </button-icon>
          <button-icon @click="playPrevTrack">
            <svg-icon icon-class="previous" />
          </button-icon>
          <button-icon class="mini-play" @click="playOrPause">
            <svg-icon :icon-class="player.playing ? 'pause' : 'play'" />
          </button-icon>
          <button-icon @click="playNextTrack">
            <svg-icon icon-class="next" />
          </button-icon>
        </div>
        <div
          class="mini-progress-track"
          :class="{ dragging: miniSeekDragging }"
          role="slider"
          tabindex="0"
          :aria-label="`播放进度 ${formatTrackTime(
            miniSeekPreview ?? player.progress
          )}`"
          :aria-valuemin="0"
          :aria-valuemax="player.currentTrackDuration"
          :aria-valuenow="Math.round(miniSeekPreview ?? player.progress)"
          @pointerdown="startMiniSeek"
          @pointermove="moveMiniSeek"
          @pointerup="finishMiniSeek"
          @pointercancel="commitMiniSeek"
          @keydown.left.prevent="nudgeMiniSeek(-5)"
          @keydown.right.prevent="nudgeMiniSeek(5)"
        >
          <div
            class="mini-progress"
            :class="{
              anon: settings.anonStyle,
              nyancat: settings.nyancatStyle,
              creeper: settings.creeperStyle,
            }"
            :style="{ width: miniProgressPercent + '%' }"
          ></div>
          <span
            v-if="
              settings.anonStyle ||
              settings.nyancatStyle ||
              settings.creeperStyle
            "
            class="mini-progress-rider"
            :class="{
              anon: settings.anonStyle,
              nyancat: settings.nyancatStyle,
              creeper: settings.creeperStyle,
              'nyancat-stop': !player.playing,
            }"
            :style="miniProgressRiderStyle"
          ></span>
        </div>
        <div
          v-if="taskbarDockHint !== 'none'"
          class="mini-dock-hint"
          :class="{ unavailable: taskbarDockHint === 'unavailable' }"
          role="status"
        >
          {{
            $t(
              taskbarDockHint === 'ready'
                ? 'taskbar.releaseToDock'
                : 'taskbar.dockUnavailable'
            )
          }}
        </div>
      </div>

      <!-- Layout containers drag the window from their blank space. -->
      <div v-if="!isMini" data-tauri-drag-region class="left-side">
        <div data-tauri-drag-region>
          <div v-if="settings.showLyricsTime" class="date">
            {{ date }}
          </div>
          <div class="cover">
            <div class="cover-container">
              <img :src="imageUrl" loading="lazy" />
              <div
                class="shadow"
                :style="{ backgroundImage: `url(${imageUrl})` }"
              ></div>
            </div>
          </div>
          <div data-tauri-drag-region class="controls">
            <div data-tauri-drag-region class="top-part">
              <div class="track-info">
                <div class="title" :title="currentTrack.name">
                  <router-link
                    v-if="hasList()"
                    :to="`${getListPath()}`"
                    @click="toggleLyrics"
                    >{{ currentTrack.name }}
                  </router-link>
                  <span v-else>
                    {{ currentTrack.name }}
                  </span>
                </div>
                <div class="subtitle">
                  <router-link
                    v-if="artist.id !== 0"
                    :to="`/artist/${artist.id}`"
                    @click="toggleLyrics"
                    >{{ artist.name }}
                  </router-link>
                  <span v-else>
                    {{ artist.name }}
                  </span>
                  <span v-if="album.id !== 0">
                    -
                    <router-link
                      :to="`/album/${album.id}`"
                      :title="album.name"
                      @click="toggleLyrics"
                      >{{ album.name }}
                    </router-link>
                  </span>
                </div>
              </div>
              <div class="top-right">
                <div class="volume-control">
                  <button-icon :title="$t('player.mute')" @click="mute">
                    <svg-icon v-show="volume > 0.5" icon-class="volume" />
                    <svg-icon v-show="volume === 0" icon-class="volume-mute" />
                    <svg-icon
                      v-show="volume <= 0.5 && volume !== 0"
                      icon-class="volume-half"
                    />
                  </button-icon>
                  <div class="volume-bar">
                    <vue-slider
                      v-model="volume"
                      :min="0"
                      :max="1"
                      :interval="0.01"
                      :drag-on-click="true"
                      :duration="0"
                      tooltip="none"
                      :dot-size="12"
                    ></vue-slider>
                  </div>
                </div>
                <div class="buttons">
                  <button-icon
                    :title="$t('player.like')"
                    @click="likeATrack(player.currentTrack.id)"
                  >
                    <svg-icon
                      :icon-class="
                        player.isCurrentTrackLiked ? 'heart-solid' : 'heart'
                      "
                    />
                  </button-icon>
                  <button-icon
                    :title="$t('contextMenu.addToPlaylist')"
                    @click="addToPlaylist"
                  >
                    <svg-icon icon-class="plus" />
                  </button-icon>
                  <!-- <button-icon @click="openMenu" title="Menu"
                    ><svg-icon icon-class="more"
                  /></button-icon> -->
                </div>
              </div>
            </div>
            <div class="progress-bar">
              <span>{{ formatTrackTime(player.progress) || '0:00' }}</span>
              <div class="slider">
                <player-progress-slider
                  :key="player.currentTrackID"
                  v-model="player.progress"
                  :min="0"
                  :max="player.currentTrackDuration"
                  :drag-on-click="true"
                  :duration="0"
                  :dot-size="12"
                  :height="2"
                  :tooltip-formatter="formatTrackTime"
                  :silent="true"
                ></player-progress-slider>
              </div>
              <span>{{ formatTrackTime(player.currentTrackDuration) }}</span>
            </div>
            <div data-tauri-drag-region class="media-controls">
              <button-icon
                v-show="!player.isPersonalFM"
                :title="
                  player.repeatMode === 'one'
                    ? $t('player.repeatTrack')
                    : $t('player.repeat')
                "
                :class="{ active: player.repeatMode !== 'off' }"
                @click="switchRepeatMode"
              >
                <svg-icon
                  v-show="player.repeatMode !== 'one'"
                  icon-class="repeat"
                />
                <svg-icon
                  v-show="player.repeatMode === 'one'"
                  icon-class="repeat-1"
                />
              </button-icon>
              <div data-tauri-drag-region class="middle">
                <button-icon
                  v-show="!player.isPersonalFM"
                  :title="$t('player.previous')"
                  @click="playPrevTrack"
                >
                  <svg-icon icon-class="previous" />
                </button-icon>
                <button-icon
                  v-show="player.isPersonalFM"
                  title="不喜欢"
                  @click="moveToFMTrash"
                >
                  <svg-icon icon-class="thumbs-down" />
                </button-icon>
                <button-icon
                  id="play"
                  :title="$t(player.playing ? 'player.pause' : 'player.play')"
                  @click="playOrPause"
                >
                  <svg-icon :icon-class="player.playing ? 'pause' : 'play'" />
                </button-icon>
                <button-icon :title="$t('player.next')" @click="playNextTrack">
                  <svg-icon icon-class="next" />
                </button-icon>
              </div>
              <button-icon
                v-show="!player.isPersonalFM"
                :title="$t('player.shuffle')"
                :class="{ active: player.shuffle }"
                @click="switchShuffle"
              >
                <svg-icon icon-class="shuffle" />
              </button-icon>
              <button-icon
                v-show="
                  isShowLyricTypeSwitch &&
                  settings.showLyricsTranslation &&
                  lyricType === 'translation'
                "
                :title="$t('player.translationLyric')"
                @click="switchLyricType"
              >
                <span class="lyric-switch-icon">译</span>
              </button-icon>
              <button-icon
                v-show="
                  isShowLyricTypeSwitch &&
                  settings.showLyricsTranslation &&
                  lyricType === 'romaPronunciation'
                "
                :title="$t('player.PronunciationLyric')"
                @click="switchLyricType"
              >
                <span class="lyric-switch-icon">音</span>
              </button-icon>
            </div>
          </div>
        </div>
      </div>
      <div v-if="!isMini" data-tauri-drag-region class="right-side">
        <transition name="slide-fade">
          <div
            v-show="!noLyric"
            ref="lyricsContainer"
            class="lyrics-container"
            :style="lyricFontSize"
            @wheel.passive="noteManualLyricScroll"
            @touchstart.passive="noteManualLyricScroll"
            @touchmove.passive="noteManualLyricScroll"
          >
            <div id="line-1" class="line"></div>
            <div
              v-for="(line, index) in lyricToShow"
              :id="`line${index}`"
              :key="index"
              class="line"
              :class="{
                highlight: highlightLyricIndex === index,
              }"
              @click="clickLyricLine(line.time)"
              @dblclick="clickLyricLine(line.time, true)"
            >
              <div class="content">
                <span
                  v-if="line.contents[0]"
                  @click.right="openLyricMenu($event, line, 0)"
                  >{{ line.contents[0] }}</span
                >
                <br />
                <span
                  v-if="line.contents[1] && settings.showLyricsTranslation"
                  class="translation"
                  @click.right="openLyricMenu($event, line, 1)"
                  >{{ line.contents[1] }}</span
                >
              </div>
            </div>
            <ContextMenu v-if="!noLyric" ref="lyricMenu">
              <div class="item" @click="copyLyric(false)">{{
                $t('contextMenu.copyLyric')
              }}</div>
              <div
                v-if="
                  rightClickLyric &&
                  rightClickLyric.contents[1] &&
                  settings.showLyricsTranslation
                "
                class="item"
                @click="copyLyric(true)"
                >{{ $t('contextMenu.copyLyricWithTranslation') }}</div
              >
            </ContextMenu>
          </div>
        </transition>
      </div>
      <div v-if="!isMini" class="close-button" @click="toggleLyrics">
        <button>
          <svg-icon icon-class="arrow-down" />
        </button>
      </div>
      <div
        v-if="!isMini"
        class="close-button"
        style="left: 24px"
        @click="fullscreen"
      >
        <button>
          <svg-icon v-if="isFullscreen" icon-class="fullscreen-exit" />
          <svg-icon v-else icon-class="fullscreen" />
        </button>
      </div>
    </div>
  </transition>
</template>

<script lang="ts">
import { defineComponent, markRaw } from 'vue';
// The lyrics page of Apple Music is so gorgeous, so I copy the design.
// Some of the codes are from https://github.com/sl1673495/vue-netease-music

import { mapActions, mapState } from 'pinia';
import { useAppStore } from '@/stores/app';
import VueSlider from 'vue-slider-component';
import ContextMenu from '@/components/ContextMenu.vue';
import PlayerProgressSlider from '@/components/PlayerProgressSlider.vue';
import { formatTrackTime } from '@/utils/common';
import { getLyric, getCloudLyric } from '@/api/track';
import {
  copyLyric,
  findActiveLyricIndex,
  hasNoLyric,
  lyricClockInterval,
  lyricParser,
  parseLyric,
  resolveLyricDisplay,
  shouldRunLyricClock,
} from '@/utils/lyrics';
import type { ParsedLyric } from '@/utils/lyrics';
import { createLyricAutoFollow, isScrollKey } from '@/utils/lyricAutoFollow';
import type { LyricAutoFollow } from '@/utils/lyricAutoFollow';
import { isEditableShortcutTarget } from '@/services/localShortcuts';
import ButtonIcon from '@/components/ButtonIcon.vue';
import Vibrant from 'node-vibrant/dist/vibrant.worker.min.js';
import Color from 'color';
import { isAccountLoggedIn } from '@/utils/auth';
import { hasListSource, getListSourcePath } from '@/utils/playList';
import locale from '@/locale';
import {
  disposeListeners,
  listen,
  startVisibilityAwareInterval,
} from '@/utils/mediaLifecycle';
import {
  invokeDesktop,
  sendDesktop,
  startDesktopWindowDragging,
} from '@/services/desktopTransport';
import { isDesktopRuntime, isTauriRuntime } from '@/utils/runtime';
import {
  calculateMiniSeekTime,
  getMiniProgressRiderStyle,
} from '@/utils/miniPlayer';
import {
  beginMiniWindowDragGesture,
  hasCrossedMiniWindowDragThreshold,
  isBarWindowSize,
  isWindowsTauri,
  shouldToggleMiniWindow,
} from '@/utils/miniWindow';
import { ARTWORK_SIZE, buildArtworkURL } from '@/utils/artwork';
import { connectTaskbarDock } from '@/services/taskbarDock';
import type {
  TaskbarDockSession,
  TaskbarPlaybackAction,
} from '@/services/taskbarDock';
import type {
  TaskbarDockHint,
  TaskbarStripSnapshot,
} from '@/services/taskbarOverlayChannel';
import { normalizePersistedLocale } from '@/locale/catalog';
import type { Artist } from '@/types/domain';

type LyricType = 'translation' | 'romaPronunciation';

interface DisplayLyricLine {
  time: number;
  content: string;
  contents: string[];
}

interface RightClickLyric extends DisplayLyricLine {
  idx: number;
}

interface MiniDragStart {
  clientX: number;
  clientY: number;
}

function formatClockTime(value: Date): string {
  const hour = value.getHours().toString().padStart(2, '0');
  const minute = value.getMinutes().toString().padStart(2, '0');
  const second = value.getSeconds().toString().padStart(2, '0');
  return `${hour}:${minute}:${second}`;
}

function mergeLyrics(
  lyrics: ParsedLyric[],
  secondaryLyrics: ParsedLyric[]
): DisplayLyricLine[] {
  return lyrics
    .filter(({ content }) => Boolean(content))
    .map(({ rawTime, time, content }) => {
      const secondary = secondaryLyrics.find(
        item => item.rawTime === rawTime
      )?.content;
      return {
        time,
        content,
        contents: secondary ? [content, secondary] : [content],
      };
    });
}

export default defineComponent({
  name: 'Lyrics',
  emits: {
    'expand-compact-window': () => true,
  },
  components: {
    VueSlider,
    PlayerProgressSlider,
    ButtonIcon,
    ContextMenu,
  },
  data() {
    return {
      lyricsIntervalCleanup: null as (() => void) | null,
      timer: null as ReturnType<typeof setInterval> | null,
      listenerCleanups: [] as Array<() => void>,
      lyric: [] as ParsedLyric[],
      tlyric: [] as ParsedLyric[],
      romalyric: [] as ParsedLyric[],
      lyricLoading: false,
      lyricRequestVersion: 0,
      lyricType: 'translation' as LyricType,
      highlightLyricIndex: -1,
      minimize: true,
      background: '',
      date: formatClockTime(new Date()),
      isFullscreen: !!document.fullscreenElement,
      rightClickLyric: null as RightClickLyric | null,
      isMini: false,
      miniTall: false,
      isAlwaysOnTop: false,
      pinDismissed: false,
      miniSeekDragging: false,
      miniSeekPreview: null as number | null,
      miniWindowDragStart: null as MiniDragStart | null,
      taskbarDock: null as TaskbarDockSession | null,
      taskbarDockHint: 'none' as TaskbarDockHint,
      taskbarDocked: false,
      lyricAutoFollow: markRaw(createLyricAutoFollow()) as LyricAutoFollow,
    };
  },
  computed: {
    ...mapState(useAppStore, ['player', 'settings', 'showLyrics', 'data']),
    // Windows mini text drags and expands the window like blank space.
    miniTextLocked() {
      return isWindowsTauri;
    },
    activeLyricIndex() {
      if (this.miniSeekDragging && typeof this.miniSeekPreview === 'number') {
        return findActiveLyricIndex(this.lyricToShow, this.miniSeekPreview);
      }
      return this.highlightLyricIndex;
    },
    // Compact mode shows only the active lyric.
    currentLyric() {
      const line = this.lyricToShow[this.activeLyricIndex];
      if (!line) return '';
      return line.contents[0] ?? '';
    },
    // Use the standard placeholder for instrumental tracks.
    displayLyric() {
      return resolveLyricDisplay(
        this.currentLyric,
        this.lyric.length,
        this.lyricLoading
      );
    },
    currentLyricTranslation() {
      const line = this.lyricToShow[this.activeLyricIndex];
      return Array.isArray(line?.contents) ? line.contents[1] : '';
    },
    // Hide translations when the compact window is too short.
    showMiniTranslation() {
      return (
        this.settings.showLyricsTranslation &&
        this.miniTall &&
        !!this.currentLyricTranslation
      );
    },
    miniProgressPercent() {
      const duration = this.player.currentTrackDuration;
      if (!duration) return 0;
      const progress = this.miniSeekPreview ?? this.player.progress;
      return Math.min(100, (progress / duration) * 100);
    },
    miniProgressRiderStyle() {
      const base = getMiniProgressRiderStyle(this.miniProgressPercent);
      if (this.settings.creeperStyle) {
        return { ...base, transform: 'translateX(-50%)' };
      }
      return base;
    },
    currentTrack() {
      return this.player.currentTrack;
    },
    volume: {
      get() {
        return this.player.volume;
      },
      set(value: number) {
        this.player.volume = value;
      },
    },
    imageUrl() {
      return buildArtworkURL(
        this.player.currentTrack?.al?.picUrl,
        ARTWORK_SIZE.lyricsCover
      );
    },
    // Request artwork sized for the 58px compact cover.
    miniImageUrl() {
      return buildArtworkURL(
        this.player.currentTrack?.al?.picUrl,
        ARTWORK_SIZE.miniPlayer
      );
    },
    bgImageUrl() {
      return buildArtworkURL(
        this.player.currentTrack?.al?.picUrl,
        ARTWORK_SIZE.lyricsBackground
      );
    },
    isShowLyricTypeSwitch() {
      return this.romalyric.length > 0 && this.tlyric.length > 0;
    },
    lyricToShow() {
      return this.lyricType === 'translation'
        ? this.lyricWithTranslation
        : this.lyricWithRomaPronunciation;
    },
    lyricWithTranslation(): DisplayLyricLine[] {
      return mergeLyrics(this.lyric, this.tlyric);
    },
    lyricWithRomaPronunciation(): DisplayLyricLine[] {
      return mergeLyrics(this.lyric, this.romalyric);
    },
    lyricFontSize() {
      return {
        fontSize: `${this.settings.lyricFontSize || 28}px`,
      };
    },
    miniLyricFontSize() {
      return {
        '--mini-lyric-size': `${this.settings.lyricFontSize || 28}px`,
      };
    },
    noLyric() {
      return hasNoLyric(this.lyric.length, this.lyricLoading);
    },
    artist(): Artist {
      return this.currentTrack.ar?.[0] ?? { id: 0, name: 'unknown' };
    },
    album() {
      return this.currentTrack?.al || { id: 0, name: 'unknown' };
    },
    theme() {
      return this.settings.lyricsBackground === true ? 'dark' : 'auto';
    },
    // What the Windows taskbar strip mirrors while the bar is docked.
    taskbarSnapshot(): TaskbarStripSnapshot | null {
      if (!this.taskbarDocked) return null;
      const duration = Math.max(0, this.player.currentTrackDuration || 0);
      const progress = Number(this.miniSeekPreview ?? this.player.progress);
      return {
        artwork: this.miniImageUrl,
        title: this.currentTrack.name ?? '',
        artist: this.artist.name ?? '',
        lyric: this.displayLyric,
        lyricLoading: this.lyricLoading,
        progress: Number.isFinite(progress)
          ? Math.min(duration, Math.max(0, progress))
          : 0,
        duration,
        playing: this.player.playing === true,
        locale: normalizePersistedLocale(this.settings.lang),
      };
    },
  },
  watch: {
    currentTrack() {
      // Clear stale lyrics before updating the tray.
      this.highlightLyricIndex = -1;
      this.lyricAutoFollow.reset();
      this.lyric = [];
      this.tlyric = [];
      this.romalyric = [];
      this.getLyric();
      this.getCoverColor();
      this.pushToTray();
    },
    // Push each active lyric to the tray.
    displayLyric() {
      this.pushToTray();
    },
    taskbarSnapshot(snapshot: TaskbarStripSnapshot | null) {
      if (snapshot) this.taskbarDock?.publish(snapshot);
    },
    showLyrics(show: boolean) {
      this.lyricAutoFollow.reset();
      if (shouldRunLyricClock(show, isDesktopRuntime)) {
        this.setLyricsInterval(show);
      } else {
        this.stopLyricsInterval();
      }
      this.enableScrollingWith(!show);
    },
  },
  created() {
    if (isWindowsTauri) {
      this.taskbarDock = markRaw(
        connectTaskbarDock({
          onHint: hint => (this.taskbarDockHint = hint),
          onDocked: docked => (this.taskbarDocked = docked),
          onAction: this.handleTaskbarAction,
          onSeek: this.handleTaskbarSeek,
        })
      );
    }
    this.getLyric();
    this.getCoverColor();
    this.initDate();
    this.listenerCleanups.push(
      listen(document, 'keydown', event =>
        this.handleLyricsKeydown(event as KeyboardEvent)
      ),
      listen(window, 'keydown', event =>
        this.noteLyricScrollKey(event as KeyboardEvent)
      ),
      listen(document, 'fullscreenchange', this.handleFullscreenChange),
      listen(window, 'resize', this.checkMini)
    );
    if (isTauriRuntime) {
      // Native fullscreen fires no fullscreenchange; it does resize.
      this.listenerCleanups.push(
        listen(window, 'resize', () => void this.syncNativeFullscreen())
      );
      void this.syncNativeFullscreen();
    }
    // Keep tray lyrics current before the watcher first runs.
    if (shouldRunLyricClock(this.showLyrics, isDesktopRuntime)) {
      this.setLyricsInterval(this.showLyrics);
    }
    this.checkMini();
    if (isDesktopRuntime) {
      invokeDesktop('isAlwaysOnTop').then(v => {
        this.isAlwaysOnTop = v === true;
      });
    }
  },
  beforeUnmount: function () {
    this.taskbarDock?.dispose();
    if (this.timer) {
      clearInterval(this.timer);
    }
    this.stopLyricsInterval();
    this.cancelMiniWindowDrag();
    disposeListeners(this.listenerCleanups);
    this.setWindowButtons(true); // Restore window controls when leaving.
  },
  unmounted() {
    this.stopLyricsInterval();
  },
  methods: {
    ...mapActions(useAppStore, [
      'toggleLyrics',
      'updateModal',
      'likeATrack',
      'enableScrollingWith',
      'fetchLikedPlaylist',
    ]),
    handleLyricsKeydown(event: KeyboardEvent) {
      if (event.key !== 'F11') return;
      event.preventDefault();
      void this.fullscreen();
    },
    noteManualLyricScroll() {
      if (this.showLyrics)
        this.lyricAutoFollow.noteManualInput(performance.now());
    },
    noteLyricScrollKey(event: KeyboardEvent) {
      if (!this.showLyrics || !isScrollKey(event.key)) return;
      if (isEditableShortcutTarget(event.target)) return;
      // Shortcut handlers may claim the key later in dispatch; only keys the
      // browser actually scrolls with count as manual input.
      setTimeout(() => {
        if (!event.defaultPrevented) this.noteManualLyricScroll();
      }, 0);
    },
    handleFullscreenChange() {
      this.isFullscreen = !!document.fullscreenElement;
    },
    async syncNativeFullscreen() {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      this.isFullscreen = await getCurrentWindow().isFullscreen();
    },
    // Switch below the compact layout breakpoint (short windows only).
    checkMini() {
      const next = isBarWindowSize({
        width: window.innerWidth,
        height: window.innerHeight,
      });
      // Two lyric lines need at least 64px of height.
      this.miniTall = window.innerHeight >= 64;
      if (next === this.isMini) return;
      this.isMini = next;
      this.pinDismissed = false;
      // Hide window controls in compact mode.
      this.setWindowButtons(!next);
      // Compact mode changes tray lyric visibility.
      this.pushToTray();
    },
    handleTaskbarAction(action: TaskbarPlaybackAction) {
      switch (action) {
        case 'previous':
          return this.playPrevTrack();
        case 'playPause':
          return this.playOrPause();
        case 'next':
          return this.playNextTrack();
      }
    },
    handleTaskbarSeek(seconds: number) {
      const duration = this.player.currentTrackDuration;
      if (duration > 0) {
        this.player.progress = Math.min(duration, Math.max(0, seconds));
      }
    },
    pushToTray() {
      if (!isDesktopRuntime) return;
      void sendDesktop('updateTrayNowPlaying', {
        // The backend also considers window visibility.
        title: this.displayLyric || this.currentTrack.name,
        isMini: this.isMini,
        // Request artwork sized for the 18px tray.
        coverUrl: buildArtworkURL(
          this.currentTrack?.al?.picUrl,
          ARTWORK_SIZE.tray
        ),
      });
    },
    setWindowButtons(visible: boolean) {
      if (!isDesktopRuntime) return;
      void sendDesktop('setWindowButtonVisibility', visible);
    },
    async toggleAlwaysOnTop() {
      if (!isDesktopRuntime) return;
      this.isAlwaysOnTop = (await invokeDesktop('toggleAlwaysOnTop')) === true;
      // Dismiss the pin before hover can reset.
      this.pinDismissed = true;
    },
    handleMiniMouseLeave() {
      this.pinDismissed = false;
      this.setWindowButtons(false);
    },
    handleMiniMouseDown(event: MouseEvent) {
      // Clear text selection before handing the gesture to Tauri.
      if (
        !beginMiniWindowDragGesture(event, window.getSelection()) ||
        !isTauriRuntime
      ) {
        return;
      }
      // Wait for movement so the first press still participates in a double-click.
      this.cancelMiniWindowDrag();
      this.miniWindowDragStart = {
        clientX: event.clientX,
        clientY: event.clientY,
      };
      document.addEventListener('mousemove', this.handleMiniDragMove);
      document.addEventListener('mouseup', this.cancelMiniWindowDrag);
    },
    handleMiniDragMove(event: MouseEvent) {
      const dragStart = this.miniWindowDragStart;
      if (!dragStart) return;
      if (!hasCrossedMiniWindowDragThreshold(dragStart, event)) {
        return;
      }
      event.preventDefault();
      this.cancelMiniWindowDrag();
      void startDesktopWindowDragging();
    },
    cancelMiniWindowDrag() {
      this.miniWindowDragStart = null;
      document.removeEventListener('mousemove', this.handleMiniDragMove);
      document.removeEventListener('mouseup', this.cancelMiniWindowDrag);
    },
    handleMiniDoubleClick(event: MouseEvent) {
      if (!shouldToggleMiniWindow(event)) return;
      event.preventDefault();
      event.stopPropagation();
      this.$emit('expand-compact-window');
    },
    updateMiniSeekPreview(event: PointerEvent) {
      const target = event.currentTarget;
      if (!(target instanceof HTMLElement)) return;
      const bounds = target.getBoundingClientRect();
      this.miniSeekPreview = calculateMiniSeekTime(
        event.clientX,
        bounds.left,
        bounds.width,
        this.player.currentTrackDuration
      );
    },
    startMiniSeek(event: PointerEvent) {
      // Clear selection before preventDefault blocks it.
      window.getSelection()?.removeAllRanges();
      event.preventDefault();
      event.stopPropagation();
      this.miniSeekDragging = true;
      if (event.currentTarget instanceof HTMLElement) {
        event.currentTarget.setPointerCapture?.(event.pointerId);
      }
      this.updateMiniSeekPreview(event);
    },
    moveMiniSeek(event: PointerEvent) {
      if (!this.miniSeekDragging) return;
      this.updateMiniSeekPreview(event);
    },
    finishMiniSeek(event: PointerEvent) {
      if (!this.miniSeekDragging) return;
      this.updateMiniSeekPreview(event);
      this.commitMiniSeek(event);
    },
    commitMiniSeek(event: PointerEvent) {
      if (!this.miniSeekDragging) return;
      const seekTime = this.miniSeekPreview;
      if (typeof seekTime === 'number' && Number.isFinite(seekTime)) {
        this.player.progress = seekTime;
        const actualSeekTime = this.player.progress;
        // Follow WKWebView's resolved seek position.
        this.highlightLyricIndex = findActiveLyricIndex(
          this.lyricToShow,
          actualSeekTime
        );
      }
      this.miniSeekDragging = false;
      this.miniSeekPreview = null;
      const target = event.currentTarget;
      if (
        target instanceof HTMLElement &&
        target.hasPointerCapture?.(event.pointerId)
      ) {
        target.releasePointerCapture(event.pointerId);
      }
    },
    nudgeMiniSeek(offset: number) {
      const duration = this.player.currentTrackDuration;
      this.player.progress = Math.min(
        duration,
        Math.max(0, this.player.progress + offset)
      );
    },
    initDate() {
      if (this.timer !== null) clearInterval(this.timer);
      this.timer = setInterval(() => {
        this.date = formatClockTime(new Date());
      }, 1000);
    },
    async fullscreen() {
      // WKWebView ships with element fullscreen disabled, so the desktop app
      // toggles the window instead.
      if (isTauriRuntime) {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const window = getCurrentWindow();
        await window.setFullscreen(!(await window.isFullscreen()));
        return;
      }
      if (document.fullscreenElement) {
        document.exitFullscreen();
      } else {
        document.documentElement.requestFullscreen();
      }
    },
    addToPlaylist() {
      if (!isAccountLoggedIn()) {
        this.showToast(locale.t('toast.needToLogin'));
        return;
      }
      this.fetchLikedPlaylist();
      this.updateModal({
        modalName: 'addTrackToPlaylistModal',
        key: 'show',
        value: true,
      });
      this.updateModal({
        modalName: 'addTrackToPlaylistModal',
        key: 'selectedTrackID',
        value: this.currentTrack?.id ?? 0,
      });
    },
    playPrevTrack() {
      this.player.playPrevTrack();
    },
    playOrPause() {
      this.player.playOrPause();
    },
    playNextTrack() {
      if (this.player.isPersonalFM) {
        this.player.playNextFMTrack();
      } else {
        this.player.playNextTrack();
      }
    },
    getLyric() {
      if (!this.currentTrack.id) {
        this.lyricLoading = false;
        return;
      }
      // Ignore responses for tracks that are no longer active.
      const requestedId = this.currentTrack.id;
      const requestVersion = ++this.lyricRequestVersion;
      this.lyricLoading = true;
      const isStale = () =>
        this.currentTrack.id !== requestedId ||
        this.lyricRequestVersion !== requestVersion;
      const finishLyricRequest = () => {
        if (!isStale()) this.lyricLoading = false;
      };
      if (
        this.currentTrack.pc !== null &&
        this.currentTrack.cd === null &&
        this.data.user?.userId
      ) {
        // Fetch embedded lyrics for unlinked cloud tracks.
        return getCloudLyric(requestedId, this.data.user?.userId)
          .then(data => {
            if (isStale()) return false;
            this.tlyric = [];
            this.romalyric = [];
            this.lyric = data.lrc?.lyric ? parseLyric(data.lrc.lyric) : [];
            this.lyricType = 'translation';
            return true;
          })
          .catch(() => {
            // A failed fetch must not leave the previous track's lyrics up.
            if (isStale()) return false;
            this.lyric = [];
            this.tlyric = [];
            this.romalyric = [];
            return false;
          })
          .finally(finishLyricRequest);
      }
      return getLyric(requestedId)
        .then(data => {
          if (isStale()) return false;
          if (!data?.lrc?.lyric) {
            this.lyric = [];
            this.tlyric = [];
            this.romalyric = [];
            return false;
          } else {
            let { lyric, tlyric, romalyric } = lyricParser(data);
            lyric = lyric.filter(
              l => !/^作(词|曲)\s*(:|：)\s*无$/.exec(l.content)
            );
            let includeAM =
              lyric.length <= 10 &&
              lyric.map(l => l.content).includes('纯音乐，请欣赏');
            if (includeAM) {
              let reg = /^作(词|曲)\s*(:|：)\s*/;
              const author = this.currentTrack.ar?.[0]?.name;
              lyric = lyric.filter(l => {
                let regExpArr = l.content.match(reg);
                return (
                  !regExpArr || l.content.replace(regExpArr[0], '') !== author
                );
              });
            }
            if (lyric.length === 1 && includeAM) {
              this.lyric = [];
              this.tlyric = [];
              this.romalyric = [];
              return false;
            } else {
              this.lyric = lyric;
              this.tlyric = tlyric;
              this.romalyric = romalyric;
              if (tlyric.length * romalyric.length > 0) {
                this.lyricType = 'translation';
              } else {
                this.lyricType =
                  lyric.length > 0 ? 'translation' : 'romaPronunciation';
              }
              return true;
            }
          }
        })
        .catch(() => {
          // A failed fetch must not leave the previous track's lyrics up.
          if (isStale()) return false;
          this.lyric = [];
          this.tlyric = [];
          this.romalyric = [];
          return false;
        })
        .finally(finishLyricRequest);
    },
    switchLyricType() {
      this.lyricType =
        this.lyricType === 'translation' ? 'romaPronunciation' : 'translation';
    },
    formatTrackTime(value: number) {
      return formatTrackTime(value);
    },
    clickLyricLine(value: number, startPlay = false) {
      // TODO: Avoid text selection on double-click lyric seeking.
      let jumpFlag = false;
      this.lyric.forEach(item => {
        if (item.content == '纯音乐，请欣赏') {
          jumpFlag = true;
        }
      });
      if ((window.getSelection()?.toString().length ?? 0) === 0 && !jumpFlag) {
        this.player.seek(value);
      }
      if (startPlay === true) {
        this.player.play();
      }
      // A clicked line is a deliberate seek; follow it right away.
      this.lyricAutoFollow.reset();
    },
    openLyricMenu(e: MouseEvent, lyric: DisplayLyricLine, idx: number) {
      this.rightClickLyric = { ...lyric, idx };
      (this.$refs['lyricMenu'] as InstanceType<typeof ContextMenu>).openMenu(e);
      e.preventDefault();
    },
    copyLyric(withTranslation: boolean) {
      if (this.rightClickLyric) {
        const idx = this.rightClickLyric.idx;
        if (!withTranslation) {
          const content = this.rightClickLyric.contents[idx];
          if (content) void copyLyric(content);
        } else {
          void copyLyric(this.rightClickLyric.contents.join(' '));
        }
      }
    },
    setLyricsInterval(showLyrics?: boolean) {
      const shouldShowLyrics = showLyrics ?? this.showLyrics;
      this.stopLyricsInterval();
      this.lyricsIntervalCleanup = startVisibilityAwareInterval(
        document,
        () => {
          if (this.player.seeking) return;
          const progress = this.player.seek() ?? 0;
          const oldHighlightLyricIndex = this.highlightLyricIndex;
          this.highlightLyricIndex = findActiveLyricIndex(
            this.lyricToShow,
            progress
          );
          if (
            shouldShowLyrics &&
            this.lyricAutoFollow.shouldScroll(
              performance.now(),
              oldHighlightLyricIndex !== this.highlightLyricIndex
            )
          ) {
            const el = document.getElementById(
              `line${this.highlightLyricIndex}`
            );
            if (el)
              el.scrollIntoView({
                behavior: 'smooth',
                block: 'center',
              });
          }
        },
        {
          foregroundMs: lyricClockInterval(shouldShowLyrics),
          backgroundMs: 250,
        }
      );
    },
    stopLyricsInterval() {
      this.lyricsIntervalCleanup?.();
      this.lyricsIntervalCleanup = null;
    },
    moveToFMTrash() {
      this.player.moveToFMTrash();
    },
    switchRepeatMode() {
      this.player.switchRepeatMode();
    },
    switchShuffle() {
      this.player.switchShuffle();
    },
    getCoverColor() {
      if (this.settings.lyricsBackground !== true) return;
      const cover = buildArtworkURL(
        this.currentTrack.al?.picUrl,
        ARTWORK_SIZE.coverColor
      );
      Vibrant.from(cover)
        .maxColorCount(1)
        .getPalette()
        .then(palette => {
          const swatch = palette.DarkMuted;
          if (!swatch) return;
          const originColor = Color.rgb(swatch.rgb);
          const color = originColor.darken(0.1).rgb().string();
          const color2 = originColor.lighten(0.28).rotate(-30).rgb().string();
          this.background = `linear-gradient(to top left, ${color}, ${color2})`;
        });
    },
    hasList() {
      return hasListSource();
    },
    getListPath() {
      return getListSourcePath();
    },
    mute() {
      this.player.mute();
    },
  },
});
</script>

<style lang="scss" scoped>
// Compact player
.mini-player {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  left: 0;
  container-type: size;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 16px 0 76px; // Reserve space for window controls.
  user-select: none;
  overflow: hidden;

  // Cover the rider and macOS resize inset with the hit area.
  --mini-rider-size: 22px;
  --mini-rider-bottom: 1px;
  --mini-progress-hit-height: 24px;

  // Keep blank container space draggable.
  .mini-copyable {
    user-select: text;
    cursor: text;
    // Keep selectable text above the progress hit area.
    position: relative;
    z-index: 2;
  }

  &.mini-text-locked .mini-copyable {
    user-select: none;
    cursor: inherit;
  }

  // Shrink with the window height.
  .mini-cover {
    height: min(58px, calc(100% - 8px));
    width: auto;
    aspect-ratio: 1;
    border-radius: 6px;
    object-fit: cover;
    flex-shrink: 0;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.24);
    -webkit-user-drag: none;
  }

  // Size to the title, capped; titles give way to lyrics when narrow.
  .mini-info {
    flex: 0 1 auto;
    max-width: 168px;
    min-width: 0;
  }

  .mini-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--color-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mini-artist {
    margin-top: 2px;
    font-size: 12px;
    color: var(--color-text);
    opacity: 0.58;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mini-lyric {
    flex: 1;
    min-width: 30%;
    text-align: center;
    color: var(--color-text);

    // The instrumental placeholder says less than the title, so it yields.
    &.placeholder {
      min-width: 0;
    }

    // Match the full lyrics view size, shrinking only when the bar is too
    // short or narrow to fit it.
    .mini-lyric-origin {
      font-size: min(var(--mini-lyric-size), 34cqh, 4.5cqw);
      font-weight: 600;
      line-height: 1.2;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }

    .mini-lyric-translation {
      margin-top: 2px;
      font-size: 14px;
      font-weight: 500;
      line-height: 1.2;
      opacity: 0.62;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
  }

  .mini-controls {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
    // Keep controls above the progress hit area.
    position: relative;
    z-index: 2;

    .svg-icon {
      width: 16px;
      height: 16px;
      color: var(--color-text);
    }

    .mini-play .svg-icon {
      width: 20px;
      height: 20px;
    }

    // Reveal the pin control only on hover.
    .mini-pin {
      margin-right: 6px;
      opacity: 0;
      transition: opacity 0.18s;

      .svg-icon {
        width: 14px;
        height: 14px;
      }

      // Color the active state only while visible.
      &.active {
        color: var(--color-primary);
      }

      &.pin-dismissed {
        opacity: 0 !important;
        pointer-events: none;
      }
    }
  }

  &:hover .mini-controls .mini-pin {
    opacity: 0.5;

    &:hover,
    &.active {
      opacity: 1;
    }
  }

  // Feedback while the bar is dragged within reach of the Windows taskbar.
  &.taskbar-snap-ready {
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.85),
      inset 0 0 12px rgba(255, 255, 255, 0.25);
  }

  .mini-dock-hint {
    position: absolute;
    top: 4px;
    left: 50%;
    z-index: 4;
    padding: 2px 8px;
    border-radius: 8px;
    background: rgba(40, 40, 40, 0.92);
    color: #eee;
    font-size: 10px;
    line-height: 14px;
    white-space: nowrap;
    pointer-events: none;
    transform: translateX(-50%);

    &.unavailable {
      color: rgba(238, 238, 238, 0.72);
    }
  }

  .mini-progress-track {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: var(--mini-progress-hit-height);
    cursor: pointer;
    touch-action: none;

    &:focus-visible {
      outline: 2px solid var(--color-primary);
      outline-offset: -2px;
    }

    // Draw only the played segment to avoid a window-edge border.
    .mini-progress {
      position: absolute;
      left: 0;
      bottom: 0;
      height: 2px;
      background-color: var(--color-primary);
      transition: width 0.4s linear;

      &.anon {
        height: 3px;
        background: linear-gradient(
          90deg,
          #ffc2d4 0%,
          #ff8fb1 60%,
          #f76d99 100%
        );
      }

      &.nyancat {
        height: 5px;
        background: linear-gradient(
          to bottom,
          #f00 0%,
          #f90 17%,
          #ff0 33%,
          #3f0 50%,
          #09f 67%,
          #63f 83%,
          #f0f 100%
        );
      }

      &.creeper {
        height: 3px;
        background: linear-gradient(
          90deg,
          #4a7c2a 0%,
          #6cb347 40%,
          #8dd968 100%
        );
      }
    }

    // Keep the rider inside the track until 100%.
    .mini-progress-rider {
      position: absolute;
      bottom: var(--mini-rider-bottom);
      width: var(--mini-rider-size);
      height: var(--mini-rider-size);
      image-rendering: pixelated;
      pointer-events: none;
      transition: left 0.4s linear, transform 0.4s linear;

      &.anon {
        background: url('/img/logos/anon.gif') center / var(--mini-rider-size)
          no-repeat;
      }

      &.nyancat {
        width: 32px;
        background: url('/img/logos/nyancat.gif') center / 32px
          var(--mini-rider-size) no-repeat;
      }

      &.nyancat.nyancat-stop {
        background-image: url('/img/logos/nyancat-stop.png');
      }

      &.creeper {
        width: 48px;
        height: 48px;
        background: url('/img/logos/creeper.gif') center / 48px 48px no-repeat;
        margin: 0 0 -6px 0;
      }
    }

    &.dragging .mini-progress,
    &.dragging .mini-progress-rider {
      transition: none;
    }
  }
}

.lyrics-page {
  position: fixed;
  top: 0;
  right: 0;
  left: 0;
  bottom: 0;
  z-index: 200;
  background: var(--color-body-bg);
  display: flex;
  clip: rect(auto, auto, auto, auto);
}

.lyrics-background {
  --contrast-lyrics-background: 75%;
  --brightness-lyrics-background: 150%;
}

[data-theme='dark'] .lyrics-background {
  --contrast-lyrics-background: 125%;
  --brightness-lyrics-background: 50%;
}

.lyrics-background {
  filter: blur(50px) contrast(var(--contrast-lyrics-background))
    brightness(var(--brightness-lyrics-background));
  position: absolute;
  height: 100vh;
  width: 100vw;

  .top-right,
  .bottom-left {
    z-index: 0;
    width: 140vw;
    height: 140vw;
    opacity: 0.6;
    position: absolute;
    background-size: cover;
  }

  .top-right {
    right: 0;
    top: 0;
    mix-blend-mode: luminosity;
  }

  .bottom-left {
    left: 0;
    bottom: 0;
    animation-direction: reverse;
    animation-delay: 10s;
  }
}

.dynamic-background > div {
  animation: rotate 150s linear infinite;
}

@keyframes rotate {
  0% {
    transform: rotate(0deg);
  }

  100% {
    transform: rotate(360deg);
  }
}

.gradient-background {
  position: absolute;
  height: 100vh;
  width: 100vw;
}

.left-side {
  flex: 1;
  display: flex;
  justify-content: flex-end;
  margin-right: 32px;
  margin-top: 24px;
  align-items: center;
  // Only the lyric/no-lyric switch animates; layout tied to the window size
  // must snap, or a live resize trails behind the window edge.
  transition: transform 0.5s, margin-right 0.5s;

  z-index: 1;

  .date {
    max-width: 54vh;
    margin: 24px 0;
    color: var(--color-text);
    text-align: center;
    font-size: 4rem;
    font-weight: 600;
    opacity: 0.88;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 1;
    overflow: hidden;
  }

  .controls {
    max-width: 54vh;
    margin-top: 24px;
    color: var(--color-text);

    .title {
      margin-top: 8px;
      font-size: 1.4rem;
      font-weight: 600;
      opacity: 0.88;
      display: -webkit-box;
      -webkit-box-orient: vertical;
      -webkit-line-clamp: 1;
      overflow: hidden;
    }

    .subtitle {
      margin-top: 4px;
      font-size: 1rem;
      opacity: 0.58;
      display: -webkit-box;
      -webkit-box-orient: vertical;
      -webkit-line-clamp: 1;
      overflow: hidden;
    }

    .top-part {
      display: flex;
      justify-content: space-between;

      .top-right {
        display: flex;
        justify-content: space-between;

        .volume-control {
          margin: 0 10px;
          display: flex;
          align-items: center;

          .volume-bar {
            width: 84px;
          }
        }

        .buttons {
          display: flex;
          align-items: center;

          button {
            margin: 0 0 0 4px;
          }

          .svg-icon {
            height: 18px;
            width: 18px;
          }
        }
      }
    }

    .progress-bar {
      margin-top: 22px;
      display: flex;
      align-items: center;
      justify-content: space-between;

      .slider {
        width: 100%;
        flex-grow: grow;
        padding: 0 10px;
      }

      span {
        font-size: 15px;
        opacity: 0.58;
        min-width: 28px;
      }
    }

    .media-controls {
      display: flex;
      justify-content: center;
      margin-top: 18px;
      align-items: center;

      button {
        margin: 0;
      }

      .svg-icon {
        opacity: 0.38;
        height: 14px;
        width: 14px;
      }

      .active .svg-icon {
        opacity: 0.88;
      }

      .middle {
        padding: 0 16px;
        display: flex;
        align-items: center;

        button {
          margin: 0 8px;
        }

        button#play .svg-icon {
          height: 28px;
          width: 28px;
          padding: 2px;
        }

        .svg-icon {
          opacity: 0.88;
          height: 22px;
          width: 22px;
        }
      }

      .lyric-switch-icon {
        color: var(--color-text);
        font-size: 14px;
        line-height: 14px;
        opacity: 0.88;
      }
    }
  }
}

.cover {
  position: relative;

  .cover-container {
    position: relative;
  }

  img {
    border-radius: 0.75em;
    width: 54vh;
    height: 54vh;
    user-select: none;
    object-fit: cover;
  }

  .shadow {
    position: absolute;
    top: 12px;
    height: 54vh;
    width: 54vh;
    filter: blur(16px) opacity(0.6);
    transform: scale(0.92, 0.96);
    z-index: -1;
    background-size: cover;
    border-radius: 0.75em;
  }
}

.right-side {
  flex: 1;
  font-weight: 600;
  color: var(--color-text);
  margin-right: 24px;
  z-index: 0;

  .lyrics-container {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding-left: 78px;
    max-width: 460px;
    overflow-y: auto;
    scrollbar-width: none; // firefox
    // macOS can't start a window drag inside a scroller, so blank space
    // falls through to .right-side; lines still catch clicks and wheel.
    pointer-events: none;

    .line {
      margin: 2px 0;
      padding: 12px 18px;
      // Not margin: the first and last lines are spaced in vh.
      transition: background-color 0.5s;
      border-radius: 12px;
      pointer-events: auto;

      &:hover {
        background: var(--color-secondary-bg-for-transparent);
      }

      .content {
        transform-origin: center left;
        transform: scale(0.95);
        transition: all 0.35s cubic-bezier(0.25, 0.46, 0.45, 0.94);
        user-select: none;

        span {
          opacity: 0.28;
          cursor: default;
          font-size: 1em;
          transition: all 0.35s cubic-bezier(0.25, 0.46, 0.45, 0.94);
        }

        span.translation {
          opacity: 0.2;
          font-size: 0.925em;
        }
      }
    }

    .line#line-1:hover {
      background: unset;
    }

    .translation {
      margin-top: 0.1em;
    }

    .highlight div.content {
      transform: scale(1);

      span {
        opacity: 0.98;
        display: inline-block;
      }

      span.translation {
        opacity: 0.65;
      }
    }
  }

  ::-webkit-scrollbar {
    display: none;
  }

  .lyrics-container .line:first-child {
    margin-top: 50vh;
  }

  .lyrics-container .line:last-child {
    margin-bottom: calc(50vh - 128px);
  }
}

.close-button {
  position: fixed;
  top: 24px;
  right: 24px;
  z-index: 300;
  border-radius: 0.75rem;
  height: 44px;
  width: 44px;
  display: flex;
  justify-content: center;
  align-items: center;
  opacity: 0.28;
  transition: 0.2s;
  -webkit-app-region: no-drag;

  .svg-icon {
    color: var(--color-text);
    padding-top: 5px;
    height: 22px;
    width: 22px;
  }

  &:hover {
    background: var(--color-secondary-bg-for-transparent);
    opacity: 0.88;
  }
}

.lyrics-page.no-lyric {
  .left-side {
    transform: translateX(27vh);
    margin-right: 0;
  }
}

// Portrait: compact header above the lyrics instead of side by side.
@media (max-aspect-ratio: 10/9) {
  .lyrics-page {
    flex-direction: column;
  }

  .left-side {
    flex: 0 0 auto;
    justify-content: center;
    align-items: stretch;
    margin: 0;
    // Clear the corner buttons and the window controls.
    padding: 76px 24px 8px;

    // Grid over the existing markup: cover | info, then progress, then transport.
    > div {
      display: grid;
      grid-template-columns: auto minmax(0, 1fr);
      column-gap: 16px;
      align-items: center;
      width: 100%;
      max-width: 560px;
    }

    .date {
      display: none;
    }

    .controls {
      display: contents;

      .title {
        margin-top: 0;
      }

      .top-part {
        grid-column: 2;
        align-items: center;
        min-width: 0;

        .track-info {
          min-width: 0;
        }

        .top-right .volume-control {
          display: none;
        }
      }

      .progress-bar {
        grid-column: 1 / -1;
        margin-top: 14px;
      }

      .media-controls {
        grid-column: 1 / -1;
        margin-top: 4px;
      }
    }
  }

  .left-side .cover {
    grid-row: 1;

    img,
    .shadow {
      width: clamp(72px, 22vw, 112px);
      height: clamp(72px, 22vw, 112px);
    }

    .shadow {
      top: 6px;
      filter: blur(10px) opacity(0.6);
    }
  }

  .right-side {
    flex: 1 1 auto;
    min-height: 0;
    margin: 0 24px;

    .lyrics-container {
      max-width: 560px;
      margin: 0 auto;
      padding-left: 0;
      // Fade lines out under the header.
      mask-image: linear-gradient(transparent, #000 48px);
    }

    // Center within the space left under the header.
    .lyrics-container .line:first-child {
      margin-top: calc(50vh - 140px);
    }

    .lyrics-container .line:last-child {
      margin-bottom: calc(50vh - 140px);
    }
  }

  // Optional: track info on top, lyrics in the middle, transport at the bottom.
  // The header containers dissolve so their children join the page grid.
  .lyrics-page.controls-bottom:not(.no-lyric) {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr) auto auto;
    column-gap: 16px;
    padding: 76px max(24px, calc((100vw - 560px) / 2)) 16px;

    .left-side,
    .left-side > div,
    .left-side .controls {
      display: contents;
    }

    .left-side .cover {
      grid-area: 1 / 1;
    }

    .left-side .controls .top-part {
      grid-area: 1 / 2;
    }

    .right-side {
      grid-area: 2 / 1 / 3 / -1;
      margin: 8px 0 0;

      .lyrics-container {
        mask-image: linear-gradient(
          transparent,
          #000 48px,
          #000 calc(100% - 48px),
          transparent
        );
      }
    }

    .left-side .controls .progress-bar {
      grid-area: 3 / 1 / 4 / -1;
      margin-top: 8px;
    }

    .left-side .controls .media-controls {
      grid-area: 4 / 1 / 5 / -1;
      margin-top: 4px;
    }
  }

  // Without lyrics, the header becomes a centered, larger now-playing card.
  .lyrics-page.no-lyric {
    .left-side {
      flex: 1 1 auto;
      transform: none;
      align-items: center;
      padding-bottom: 24px;

      > div {
        grid-template-columns: minmax(0, 1fr);
        justify-items: center;
        max-width: min(78vw, 46vh);
      }

      .date {
        display: -webkit-box;
        margin: 0 0 16px;
        font-size: 2.6rem;
      }

      .cover {
        grid-row: auto;
      }

      .cover img,
      .cover .shadow {
        width: min(78vw, 46vh);
        height: min(78vw, 46vh);
      }

      .cover .shadow {
        top: 12px;
        filter: blur(16px) opacity(0.6);
      }

      .controls .top-part {
        grid-column: 1;
        grid-row: auto;
        width: 100%;
        margin-top: 24px;
      }

      .controls .progress-bar {
        width: 100%;
      }
    }

    .right-side {
      display: none;
    }
  }
}

@media screen and (min-width: 1200px) {
  .right-side .lyrics-container {
    max-width: 600px;
  }
}

.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.4s;
}

.slide-up-enter-from,
.slide-up-leave-to

/* .fade-leave-active below version 2.1.8 */ {
  transform: translateY(100%);
}

.slide-fade-enter-active {
  transition: all 0.5s ease;
}

.slide-fade-leave-active {
  transition: all 0.5s cubic-bezier(0.2, 0.2, 0, 1);
}

.slide-fade-enter-from,
.slide-fade-leave-to {
  transform: translateX(27vh);
  opacity: 0;
}
</style>
