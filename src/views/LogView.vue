<script setup lang="ts">
// ============================================================================
// LogView —— 运行日志页
//
// 展示后端持久化的运行日志文件（logger.rs 按天写入 {config_dir}/LarkReader/logs）。
// 下载任务（开始 / 逐项结果 / 汇总 / 失败）、登录登出、设置变更等关键事件都会
// 落在这里，方便回看「下载了什么、花了多久、出了什么问题」。
//
// 交互：
//   - 顶部按日期切换日志文件（今天默认在最前）
//   - 右侧工具栏：关键字过滤 / 自动刷新（默认开）/ 手动刷新
//   - 自动刷新时若停在底部则跟随滚动到最新一条
// ============================================================================

import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import * as logApi from "../api/log";
import type { LogFileMeta } from "../api/types";
import AppIcon from "../components/AppIcon.vue";

const files = ref<LogFileMeta[]>([]);
const activeName = ref<string | null>(null);
const content = ref("");
const truncated = ref(false);
const errorText = ref("");

const follow = ref(true);
const busy = ref(false);
const filterText = ref("");

const rowsEl = ref<HTMLElement | null>(null);
let stickToBottom = true;

const activeMeta = computed(
  () => files.value.find((file) => file.name === activeName.value) ?? null
);

const displayLines = computed(() => {
  const lines = content.value.split("\n");
  const keyword = filterText.value.trim();
  if (!keyword) return lines;
  const lower = keyword.toLowerCase();
  return lines.filter((line) => line.toLowerCase().includes(lower));
});

const filtering = computed(() => filterText.value.trim().length > 0);

const followLabel = computed(() => (follow.value ? "自动刷新：开" : "自动刷新：关"));

/** 文件 chip 文案：app-2026-09-05.log -> 2026-09-05 */
function fileLabel(name: string): string {
  return name.replace(/^app-/, "").replace(/\.log$/, "");
}

function fmtBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
}

function fmtClock(iso: string | null): string {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

/** 行级别（用于着色） */
function lineLevel(line: string): string {
  if (line.includes(" [WARN] ")) return "warn";
  if (line.includes(" [ERROR] ")) return "error";
  return "info";
}

async function loadFilesList() {
  files.value = await logApi.listLogFiles();
  const exists = files.value.some((file) => file.name === activeName.value);
  if (!exists) {
    activeName.value = files.value[0]?.name ?? null;
  }
}

async function loadActiveContent() {
  if (!activeName.value) {
    content.value = "";
    return;
  }
  const data = await logApi.readLogFile(activeName.value);
  truncated.value = data.truncated;
  if (content.value !== data.content) {
    content.value = data.content;
  }
}

async function refreshAll(silent = false) {
  if (!silent) busy.value = true;
  errorText.value = "";
  try {
    await loadFilesList();
    await loadActiveContent();
  } catch (err) {
    errorText.value = String(err);
    content.value = "";
  } finally {
    if (!silent) busy.value = false;
  }
}

/** 选择某一天的日志文件（选中后暂停自动刷新，避免打扰阅读旧日志） */
async function selectFile(name: string) {
  if (activeName.value === name) return;
  activeName.value = name;
  if (follow.value) follow.value = false;
  content.value = "";
  await refreshAll(true);
}

async function toggleFollow() {
  follow.value = !follow.value;
}

function scrollToBottom() {
  const el = rowsEl.value;
  if (!el) return;
  el.scrollTop = el.scrollHeight;
}

function onRowsScroll() {
  const el = rowsEl.value;
  if (!el) return;
  stickToBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 96;
}

async function openLogDir() {
  try {
    await logApi.openLogDir();
  } catch (err) {
    errorText.value = String(err);
  }
}

// 自动刷新：跟随文件内容增长
let timer: number | undefined;
function syncTimer() {
  if (timer !== undefined) {
    window.clearInterval(timer);
    timer = undefined;
  }
  if (follow.value) {
    timer = window.setInterval(() => {
      void refreshAll(true);
    }, 2000);
  }
}

watch(follow, syncTimer);

// 内容变化且停留在底部时，跟随滚动到最新
watch(content, async () => {
  if (follow.value && !filtering.value && stickToBottom) {
    await nextTick();
    scrollToBottom();
  }
});

onMounted(async () => {
  await refreshAll(true);
  syncTimer();
  await nextTick();
  scrollToBottom();
});

onBeforeUnmount(() => {
  if (timer !== undefined) window.clearInterval(timer);
});
</script>

<template>
  <div class="lr-page">
    <header class="lr-page__head">
      <h1 class="lr-page__title">运行日志</h1>
      <p class="lr-page__desc">
        下载任务、登录登出、设置变更等关键事件都会记到这里，按天保存在本地，保留最近 30
        天。正在执行的任务会实时写入，可直接观察下载效果与耗时。
      </p>
    </header>

    <div class="lr-page__body lr-log">
      <!-- 文件选择 -->
      <section class="lr-card">
        <header class="lr-card__head">
          <span class="lr-card__title">日志文件</span>
          <button class="lr-btn lr-btn--secondary lr-log__opendir" @click="openLogDir">
            打开日志目录
          </button>
        </header>
        <div class="lr-card__body lr-log__filebar">
          <p v-if="files.length === 0" class="lr-log__hint">
            还没有日志文件。进行一次下载或登录操作后，这里会自动出现当天日志。
          </p>
          <button
            v-for="file in files"
            v-else
            :key="file.name"
            class="lr-log__filechip"
            :class="{ 'is-active': file.name === activeName }"
            :title="`${fmtBytes(file.size_bytes)} · 更新于 ${fmtClock(file.modified_at)}`"
            @click="selectFile(file.name)"
          >
            {{ fileLabel(file.name) }}
          </button>
        </div>
      </section>

      <!-- 日志内容 -->
      <section class="lr-card lr-log__viewer">
        <header class="lr-card__head lr-log__toolbar">
          <span class="lr-card__title lr-log__activefile">
            {{ activeName ? fileLabel(activeName) : "（无日志）" }}
            <span v-if="activeMeta" class="lr-card__meta lr-log__meta">
              {{ fmtBytes(activeMeta.size_bytes) }} · {{ fmtClock(activeMeta.modified_at) }}
            </span>
          </span>
          <span class="lr-log__tools">
            <input
              v-model="filterText"
              class="lr-log__filter"
              type="search"
              placeholder="过滤关键字…"
            />
            <button
              class="lr-btn lr-btn--secondary lr-log__followbtn"
              :class="{ 'is-on': follow }"
              :aria-pressed="follow"
              @click="toggleFollow"
            >
              {{ followLabel }}
            </button>
            <button
              class="lr-btn lr-btn--secondary"
              :disabled="busy"
              @click="refreshAll(false)"
            >
              <AppIcon v-if="busy" name="spinner" :size="12" class="lr-log__spin" />
              {{ busy ? "刷新中…" : "刷新" }}
            </button>
          </span>
        </header>

        <div class="lr-log__status" v-if="errorText">
          <AppIcon name="alert-circle" :size="13" />
          {{ errorText }}
        </div>

        <div ref="rowsEl" class="lr-card__body lr-log__body" @scroll.passive="onRowsScroll">
          <template v-if="content">
            <p v-if="displayLines.length === 0" class="lr-log__hint">
              没有匹配「{{ filterText }}」的行
            </p>
            <template v-else>
              <p
                v-for="(line, index) in displayLines"
                :key="index"
                class="lr-log__line"
                :class="`is-${lineLevel(line)}`"
              >
                {{ line }}
              </p>
            </template>
          </template>
          <div v-else class="lr-empty">
            <AppIcon name="log" :size="28" />
            <p>{{ errorText ? "日志读取失败，请稍后重试" : "这里将显示应用运行日志" }}</p>
          </div>
          <p v-if="truncated" class="lr-log__truncated">
            文件较大，仅显示末尾部分（{{ activeMeta ? fmtBytes(activeMeta.size_bytes) : "" }}）。
          </p>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.lr-log {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-4);
}

/* ---- 文件选择条 ---- */
.lr-log__filebar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--lr-space-2);
}

.lr-log__filechip {
  padding: var(--lr-space-1) var(--lr-space-3);
  border-radius: 999px;
  border: 0.5px solid var(--lr-border);
  background: var(--lr-bg-subtle);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease, border-color 0.15s ease;
}

.lr-log__filechip:hover {
  border-color: var(--lr-primary);
  color: var(--lr-primary);
}

.lr-log__filechip.is-active {
  background: var(--lr-primary-soft);
  border-color: var(--lr-primary);
  color: var(--lr-primary);
  font-weight: var(--lr-fw-medium);
}

.lr-log__opendir {
  height: 26px;
  padding: 0 var(--lr-space-3);
  font-size: var(--lr-fs-secondary);
}

/* ---- 查看器 ---- */
.lr-log__viewer {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.lr-log__toolbar {
  flex: none;
}

.lr-log__activefile {
  display: inline-flex;
  align-items: baseline;
  gap: var(--lr-space-2);
  min-width: 0;
}

.lr-log__meta {
  font-weight: var(--lr-fw-regular);
}

.lr-log__tools {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  min-width: 0;
}

.lr-log__filter {
  width: 180px;
  height: 26px;
  padding: 0 var(--lr-space-3);
  border-radius: var(--lr-radius-md);
  border: 0.5px solid var(--lr-border);
  background: var(--lr-bg-input, var(--lr-bg-subtle));
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text);
}

.lr-log__filter:focus {
  outline: none;
  border-color: var(--lr-primary);
}

.lr-log__followbtn {
  height: 26px;
  padding: 0 var(--lr-space-3);
  font-size: var(--lr-fs-secondary);
}

.lr-log__followbtn.is-on {
  background: var(--lr-primary-soft);
  border-color: var(--lr-primary);
  color: var(--lr-primary);
}

.lr-log__body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  margin: 0 var(--lr-space-4) var(--lr-space-4);
  padding: var(--lr-space-3) var(--lr-space-4);
  border-radius: var(--lr-radius-md);
  background: var(--lr-bg-subtle);
  font-family: var(--lr-font-mono);
  font-size: var(--lr-fs-mono);
  line-height: var(--lr-lh-body);
}

.lr-log__line {
  margin: 0;
  color: var(--lr-text-secondary);
  word-break: break-word;
  white-space: pre-wrap;
}

.lr-log__line.is-warn {
  color: var(--lr-warning);
}

.lr-log__line.is-error {
  color: var(--lr-danger);
}

.lr-log__status {
  flex: none;
  display: flex;
  align-items: flex-start;
  gap: var(--lr-space-2);
  margin: 0 var(--lr-space-4) var(--lr-space-3);
  padding: var(--lr-space-2) var(--lr-space-3);
  border-radius: var(--lr-radius-md);
  background: var(--lr-danger-soft);
  color: var(--lr-danger);
  font-size: var(--lr-fs-secondary);
  word-break: break-all;
}

.lr-log__hint {
  margin: 0;
  color: var(--lr-text-tertiary);
}

.lr-log__truncated {
  margin: var(--lr-space-2) 0 0;
  color: var(--lr-warning);
  font-size: var(--lr-fs-secondary);
  font-family: var(--lr-font-body);
}

.lr-log__spin {
  animation: lr-log-spin 0.9s linear infinite;
}

@keyframes lr-log-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .lr-log__spin {
    animation: none;
  }
}
</style>
