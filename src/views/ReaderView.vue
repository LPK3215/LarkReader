<script setup lang="ts">
// ============================================================================
// ReaderView —— 本地阅读页（/reader）
//
// 定位：把「已落盘的知识库导出目录」渲染成离线可读的本地阅读器。
//   - 阅读源：最近一次任务产物目录 / 任务历史中的产物目录 / 手动选择文件夹
//   - 左：文件系统目录树（= 知识库层级，惰性加载）
//   - 右：markdown 渲染正文；图片相对路径按「md 同目录」解析回本地并 base64
//         内联（CSP 已放行 data:），做到离线可读。
//
// 数据：src/api/reader.ts（list_reader_dir / read_reader_md / read_reader_binary）
// ============================================================================

import { computed, nextTick, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import MarkdownIt from "markdown-it";
import { open } from "@tauri-apps/plugin-dialog";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";

import AppIcon from "../components/AppIcon.vue";
import ReaderTree from "../components/ReaderTree.vue";
import {
  findFirstReaderDoc,
  readReaderBinary,
  readReaderMd,
  readReaderText,
} from "../api/reader";
import { useTaskStore } from "../stores/task";
import { useHistoryStore } from "../stores/history";
import { openOutputDir } from "../api/settings";
import { message } from "../composables/useMessage";

const route = useRoute();
const router = useRouter();

const task = useTaskStore();
const history = useHistoryStore();

// ---- markdown 渲染器（本地内容受信任，关闭 raw html） ----
const md = new MarkdownIt({ html: false, linkify: true, breaks: true });

// ============================================================================
// 状态
// ============================================================================

/** 当前阅读源根目录；null = 尚未选择（空态选源） */
const rootPath = ref<string | null>(null);
const rootName = ref("");
/** 正在阅读的文档（md）绝对路径 */
const docPath = ref<string | null>(null);

/** 需要树自动展开定位到的文档（任务历史「应用内阅读」直达时设置） */
const revealPath = ref<string | null>(null);

const mdLoading = ref(false);
const mdError = ref<string | null>(null);
const contentHtml = ref("");
const contentEl = ref<HTMLElement | null>(null);

/** 正文呈现形态：md 渲染 / 图片 / PDF 内嵌 / 纯文本 / 其他（交给系统打开） */
const viewKind = ref<"md" | "image" | "pdf" | "text" | "other">("md");
const binaryUrl = ref("");
const textContent = ref("");

/** 应用内可预览的扩展名分组（与后端 reader.rs 的白名单保持一致） */
const IMAGE_EXTS = new Set(["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif", "ico"]);
const TEXT_EXTS = new Set([
  "txt", "csv", "tsv", "json", "ndjson", "xml", "log", "yaml", "yml", "toml",
  "html", "htm", "ini", "cfg", "conf", "sql", "sh", "bat", "ps1", "py", "js",
  "ts", "css", "java", "c", "h", "cpp", "go", "rs", "rb",
]);

function extOf(path: string): string {
  const name = basename(path);
  const dot = name.lastIndexOf(".");
  return dot >= 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/**
 * 图片 data URL 缓存（同一资源只读一次）。
 * Map 保持插入顺序，超过上限时淘汰最旧条目，避免长会话/大库把大量 base64 常驻内存。
 */
const imageCache = new Map<string, string>();
const IMAGE_CACHE_MAX = 200;

function cacheImage(key: string, dataUrl: string) {
  if (imageCache.has(key)) imageCache.delete(key); // 重新命中视为“最近使用”，挪到末尾
  imageCache.set(key, dataUrl);
  if (imageCache.size > IMAGE_CACHE_MAX) {
    const oldest = imageCache.keys().next().value;
    if (oldest !== undefined) imageCache.delete(oldest);
  }
}

/** 最近一次任务产物目录（本会话还没跑过任务则为空） */
const lastTaskDir = computed<string | null>(() => task.outputRoot || null);

/** 历史任务里有产物目录的项（Reader 的可选阅读源） */
const historyDirs = computed(() =>
  history.records
    .filter((r) => r.result?.output_root)
    .map((r) => ({
      task_id: r.task_id,
      name: r.result?.wiki_name || "未命名的导出",
      path: r.result!.output_root,
    }))
);

const sources = computed(() => {
  const out: { key: string; name: string; path: string; source: string }[] = [];
  // 同一产物目录去重（最近一次任务与任务历史可能指向同一目录），避免出现两行重复入口
  const seen = new Set<string>();
  const push = (
    key: string,
    name: string,
    path: string,
    source: string
  ) => {
    if (seen.has(path)) return;
    seen.add(path);
    out.push({ key, name, path, source });
  };
  if (lastTaskDir.value) {
    push("latest", basename(lastTaskDir.value), lastTaskDir.value, "最近一次任务");
  }
  for (const item of historyDirs.value) {
    push(item.task_id, item.name, item.path, "任务历史");
  }
  return out;
});

// ============================================================================
// 动作
// ============================================================================

async function loadHistory() {
  if (history.records.length === 0 && !history.loading) {
    await history.load();
  }
}
loadHistory();

function setRoot(path: string) {
  rootPath.value = path;
  rootName.value = basename(path);
  docPath.value = null;
  revealPath.value = null;
  contentHtml.value = "";
  mdError.value = null;
}

async function pickFolder() {
  try {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string" && selected) setRoot(selected);
  } catch (err) {
    // 用户取消时 dialog 返回 null 不会走到这；真错误用 toast 提示即可
    message.warning(`选择目录失败：${String(err)}`);
  }
}

function clearRoot() {
  rootPath.value = null;
  rootName.value = "";
  docPath.value = null;
  revealPath.value = null;
  contentHtml.value = "";
  mdError.value = null;
}

/** 打开文件：按扩展名分发 —— md 渲染 / 图片 / PDF 内嵌 / 纯文本 / 其他交给系统 */
async function openDoc(path: string) {
  if (path === docPath.value) return;
  const requested = path;
  mdLoading.value = true;
  mdError.value = null;
  docPath.value = path;
  revealPath.value = path;
  contentHtml.value = "";
  textContent.value = "";
  binaryUrl.value = "";

  const ext = extOf(path);
  const kind: typeof viewKind.value = IMAGE_EXTS.has(ext)
    ? "image"
    : ext === "pdf"
      ? "pdf"
      : ext === "md" || ext === "markdown"
        ? "md"
        : TEXT_EXTS.has(ext)
          ? "text"
          : "other";
  viewKind.value = kind;

  try {
    if (kind === "md") {
      const raw = await readReaderMd(path);
      if (docPath.value !== requested) return; // 等待期间用户已切到别的文档，丢弃这次结果
      contentHtml.value = md.render(raw);
    } else if (kind === "image" || kind === "pdf") {
      const bin = await readReaderBinary(path);
      if (docPath.value !== requested) return;
      binaryUrl.value = bin.data_url;
    } else if (kind === "text") {
      const raw = await readReaderText(path);
      if (docPath.value !== requested) return;
      textContent.value = raw;
    }
  } catch (err) {
    if (docPath.value !== requested) return;
    mdError.value = String(err);
  } finally {
    if (docPath.value !== requested) return;
    mdLoading.value = false;
    if (kind === "md") {
      void nextTick(() => {
        resolveLocalImages();
        const el = contentEl.value;
        if (el) el.scrollTop = 0; // 切换文档后回到顶部，避免停留在上一篇的滚动位置
      });
    }
  }
}

/** 其他格式：交给系统默认程序打开 */
async function openCurrentWithSystem() {
  if (!docPath.value) return;
  try {
    await openPath(docPath.value);
  } catch (err) {
    message.warning(`用系统程序打开失败：${String(err)}`);
  }
}

/**
 * 任务历史「应用内阅读」直达入口：route query.dir 指向产物目录时，
 * 自动切到该阅读源并打开第一篇 md（树随之展开定位）。
 */
async function openExternalSource(dir: string) {
  setRoot(dir);
  let first: string | null = null;
  try {
    first = await findFirstReaderDoc(dir);
  } catch (err) {
    // 树仍可手动浏览目录，不必占据正文区
    message.warning(`定位第一篇文档失败：${String(err)}`);
  }
  if (first) {
    revealPath.value = first;
    await openDoc(first);
  }
  // 消费后清掉 query：否则用户手动更换阅读源后再点同一条历史不会触发
  if (route.query.dir) {
    await router.replace({ path: "/reader", query: {} });
  }
}

/** 把 md 里相对路径的 <img src> 换成本地文件 data URL（按 md 同目录解析）。
 *  图片多时串行 IPC 会很慢，这里用固定小并发批量读取；文档切换后立即停止。
 */
async function resolveLocalImages() {
  const container = contentEl.value;
  const current = docPath.value;
  if (!container || !current) return;
  const baseDir = dirname(current);
  const imgs = Array.from(container.querySelectorAll<HTMLImageElement>("img"));
  const jobs = imgs.map((img) => ({ img, src: img.getAttribute("src") || "" }));
  const pending = jobs.filter(({ src }) => !/^(data:|https?:|asset:|blob:)/i.test(src));

  const CONCURRENCY = 6;
  let next = 0;
  async function worker() {
    while (docPath.value === current) {
      const job = pending[next++];
      if (!job) return;
      const { img, src } = job;
      let rel = src;
      try {
        rel = decodeURIComponent(src);
      } catch {
        /* 保留原样 */
      }
      const abs = joinPath(baseDir, rel);
      try {
        const cached = imageCache.get(abs);
        const dataUrl = cached ?? (await readReaderBinary(abs)).data_url;
        if (!cached) cacheImage(abs, dataUrl);
        img.src = dataUrl;
      } catch (err) {
        img.classList.add("is-broken");
        img.alt = img.alt ? `${img.alt}（图片缺失）` : "（图片缺失）";
        img.title = String(err);
      }
    }
  }
  await Promise.all(
    Array.from(
      { length: Math.min(CONCURRENCY, Math.max(1, pending.length)) },
      () => worker()
    )
  );
}

/** 阅读区链接点击：外部 http(s) 链接交给系统浏览器打开；
 *  本地相对链接一律阻止默认跳转（避免跑到 tauri:// 空白页）。 */
function onContentClick(event: MouseEvent) {
  const target = (event.target as HTMLElement).closest("a");
  if (!target) return;
  const href = target.getAttribute("href") || "";
  if (/^(https?:)/i.test(href)) {
    event.preventDefault();
    void openUrl(href).catch(() => {
      /* 打不开外部链接时保持安静，避免打断阅读 */
    });
    return;
  }
  if (/^(data:|mailto:|asset:)/i.test(href)) return;
  event.preventDefault();
}

/** 在系统文件管理器里显示当前文档所在目录 */
async function revealDoc() {
  if (!rootPath.value && !docPath.value) return;
  // 正在读文件时打开其所在目录；否则打开当前阅读源根目录
  const target = docPath.value ? dirname(docPath.value) : (rootPath.value as string);
  try {
    await openOutputDir(target);
  } catch (err) {
    // 系统级失败（目录已被删/权限）不该覆盖正在阅读的正文
    message.warning(`打开目录失败：${String(err)}`);
  }
}

// 切换阅读源时刷新历史（保持右侧「已有导出」清单最新）
watch(rootPath, (path) => {
  if (path) void history.refresh();
});

// 任务历史「应用内阅读」直达：?dir=<产物目录>
watch(
  () => route.query.dir,
  (dir) => {
    if (typeof dir === "string" && dir) void openExternalSource(dir);
  },
  { immediate: true }
);

// ============================================================================
// 纯路径工具（避免引 node:path —— Tauri 前端不保证可用）
// ============================================================================

function basename(path: string): string {
  const idx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return idx >= 0 ? path.slice(idx + 1) : path;
}

function dirname(path: string): string {
  const idx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return idx > 0 ? path.slice(0, idx) : path;
}

function joinPath(dir: string, rel: string): string {
  const d = dir.replace(/\\/g, "/").replace(/\/+$/, "");
  const r = rel.replace(/\\/g, "/");
  return `${d}/${r}`;
}
</script>

<template>
  <div class="lr-page">
    <header class="lr-page__head lr-reader__head">
      <div>
        <h1 class="lr-page__title">本地阅读</h1>
        <p class="lr-page__desc">
          离线阅读已导出的文档
        </p>
      </div>
      <div class="lr-reader__head-ops">
        <button v-if="rootPath" class="lr-btn lr-btn--secondary" @click="revealDoc">
          <AppIcon name="folder-open" :size="14" />
          打开所在目录
        </button>
        <button class="lr-btn lr-btn--secondary" @click="clearRoot">
          <AppIcon name="refresh" :size="14" />
          更换阅读源
        </button>
      </div>
    </header>

    <div class="lr-page__body">
      <!-- 空态：选择阅读源 -->
      <div v-if="!rootPath" class="lr-reader__choose">
        <div class="lr-empty lr-empty--reader">
          <AppIcon name="book" :size="30" />
          <span>选择一份导出开始阅读</span>
        </div>

        <div v-if="sources.length" class="lr-card lr-reader__sources">
          <div class="lr-card__head">
            <span class="lr-card__title">已有导出</span>
            <span class="lr-card__meta">最近 24 小时的任务产物</span>
          </div>
          <ul class="lr-reader__source-list">
            <li
              v-for="s in sources"
              :key="s.key"
              class="lr-reader__source-row"
              @click="setRoot(s.path)"
            >
              <AppIcon name="book" :size="16" class="lr-reader__source-icon" />
              <div class="lr-reader__source-main">
                <span class="lr-reader__source-name">{{ s.name }}</span>
                <code class="lr-reader__source-path">{{ s.path }}</code>
              </div>
              <span class="lr-badge lr-badge--info">{{ s.source }}</span>
            </li>
          </ul>
        </div>

        <div class="lr-card lr-reader__choose-other">
          <button class="lr-btn lr-btn--primary lr-btn--lg" @click="pickFolder">
            <AppIcon name="folder-open" :size="15" />
            选择其他文件夹
          </button>
          <p class="lr-reader__choose-hint">导出后回这里直接读</p>
        </div>
      </div>

      <!-- 浏览/阅读态 -->
      <div v-else class="lr-reader__split">
        <aside class="lr-card lr-reader__side">
          <div class="lr-card__head lr-reader__side-head">
            <span class="lr-card__title lr-reader__side-title" :title="rootPath">
              {{ rootName }}
            </span>
          </div>
          <div class="lr-reader__side-body">
            <ReaderTree
              :root-path="rootPath"
              :active-path="docPath"
              :reveal-path="revealPath"
              @select="openDoc"
            />
          </div>
        </aside>

        <main class="lr-card lr-reader__main">
          <!-- 加载中 -->
          <div v-if="mdLoading" class="lr-empty">
            <AppIcon name="spinner" :size="24" class="lr-icon-spin" />
            <span>正在渲染文档…</span>
          </div>

          <!-- 报错 -->
          <div v-else-if="mdError" class="lr-reader__error">
            <AppIcon name="alert-circle" :size="16" />
            {{ mdError }}
          </div>

          <!-- 未选文档 -->
          <div v-else-if="!docPath" class="lr-empty">
            <AppIcon name="doc" :size="24" />
            <span>选择文档开始阅读</span>
          </div>

          <!-- markdown 正文 -->
          <article
            v-else-if="viewKind === 'md'"
            ref="contentEl"
            class="lr-reader-md"
            @click="onContentClick"
          >
            <!-- eslint-disable-next-line vue/no-v-html -->
            <div v-html="contentHtml" />
          </article>

          <!-- 非 md 附件：文件名信息头 + 内容 -->
          <template v-else>
            <div class="lr-reader__filehead">
              <AppIcon name="paperclip" :size="13" />
              <span class="lr-reader__filename">{{ docPath ? basename(docPath) : "" }}</span>
              <button
                class="lr-btn lr-btn--ghost lr-reader__fileopen"
                title="用系统默认程序打开"
                @click="openCurrentWithSystem"
              >
                <AppIcon name="folder-open" :size="13" />
                系统打开
              </button>
            </div>

            <!-- 图片附件（1x1 像素测试图也会按最小尺寸放大显示） -->
            <div v-if="viewKind === 'image'" class="lr-reader-media">
              <img :src="binaryUrl" :alt="docPath ? basename(docPath) : ''" />
            </div>

            <!-- PDF 附件 -->
            <div v-else-if="viewKind === 'pdf'" class="lr-reader-media lr-reader-media--fill">
              <embed :src="binaryUrl" type="application/pdf" />
            </div>

            <!-- 纯文本附件 -->
            <pre v-else-if="viewKind === 'text'" class="lr-reader-code">{{ textContent }}</pre>

            <!-- 其他格式：交给系统默认程序 -->
            <div v-else class="lr-reader-other">
              <AppIcon name="paperclip" :size="28" />
              <p class="lr-reader-other__hint">暂不支持预览</p>
              <button class="lr-btn lr-btn--secondary" @click="openCurrentWithSystem">
                <AppIcon name="folder-open" :size="14" />
                系统打开
              </button>
            </div>
          </template>
        </main>
      </div>
    </div>
  </div>
</template>

<style scoped>
.lr-reader__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--lr-space-4);
}

.lr-reader__head-ops {
  display: flex;
  gap: var(--lr-space-2);
  flex: none;
}

/* ---- 空态选源 ---- */
.lr-reader__choose {
  height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: var(--lr-space-5);
  max-width: 680px;
  margin: 0 auto;
  padding: var(--lr-space-6) var(--lr-space-2) var(--lr-space-6);
}

.lr-empty--reader {
  margin: var(--lr-space-4) 0;
}

.lr-reader__sources {
  overflow: hidden;
}

.lr-reader__source-list {
  list-style: none;
  margin: 0;
  padding: 0;
  max-height: 320px;
  overflow: auto;
}

.lr-reader__source-row {
  display: flex;
  align-items: center;
  gap: var(--lr-space-3);
  padding: var(--lr-space-3) var(--lr-space-4);
  cursor: pointer;
  transition: background 0.15s;
}

.lr-reader__source-row:hover {
  background: var(--lr-bg-subtle);
}

.lr-reader__source-icon {
  flex: none;
  color: var(--lr-primary);
}

.lr-reader__source-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.lr-reader__source-name {
  font-weight: var(--lr-fw-medium);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.lr-reader__source-path {
  font-size: var(--lr-fs-mono);
  color: var(--lr-text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.lr-reader__choose-other {
  display: flex;
  align-items: center;
  gap: var(--lr-space-3);
  padding: var(--lr-space-4);
}

.lr-reader__choose-hint {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

/* ---- 浏览/阅读布局 ---- */
.lr-reader__split {
  height: 100%;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(260px, 320px) minmax(0, 1fr);
  gap: var(--lr-space-4);
}

.lr-reader__side {
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.lr-reader__side-head {
  flex: none;
}

.lr-reader__side-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--lr-fs-section);
}

.lr-reader__side-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: var(--lr-space-2);
}

.lr-reader__main {
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.lr-reader__error {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  margin: var(--lr-space-5);
  padding: var(--lr-space-3) var(--lr-space-4);
  border-radius: var(--lr-radius-md);
  background: var(--lr-danger-soft);
  border: 0.5px solid var(--lr-danger-border);
  color: var(--lr-danger);
  font-size: var(--lr-fs-secondary);
}

/* ---- 非 md 附件的阅读形态 ---- */
.lr-reader__filehead {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  padding: var(--lr-space-3) var(--lr-space-4);
  border-bottom: 1px solid var(--lr-border);
  color: var(--lr-text-secondary);
  font-size: var(--lr-fs-secondary);
}

.lr-reader__filename {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--lr-text);
  font-weight: var(--lr-fw-medium);
}

.lr-reader__fileopen {
  flex: none;
  height: 24px;
  padding: 0 var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
}

.lr-reader-media {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--lr-space-4);
}

.lr-reader-media--fill {
  padding: 0;
}

.lr-reader-media img {
  max-width: 100%;
  max-height: 100%;
  border-radius: var(--lr-radius-md);
  /* 极小图（如 1x1 像素测试图）也可见：按最小显示尺寸放大并保留像素感 */
  min-width: 48px;
  min-height: 48px;
  image-rendering: pixelated;
}

.lr-reader-media embed {
  width: 100%;
  height: 100%;
  border: none;
}

.lr-reader-code {
  flex: 1;
  min-height: 0;
  overflow: auto;
  margin: 0;
  padding: var(--lr-space-5);
  font-family: var(--lr-font-mono, ui-monospace, monospace);
  font-size: 13px;
  line-height: 1.6;
  color: var(--lr-text);
  white-space: pre-wrap;
  word-break: break-word;
  background: var(--lr-bg-code, rgba(127, 127, 127, 0.08));
}

.lr-reader-other {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--lr-space-3);
  padding: var(--lr-space-6);
  color: var(--lr-text-secondary);
}

.lr-reader-other__name {
  font-weight: var(--lr-fw-medium);
  color: var(--lr-text);
  word-break: break-all;
}

.lr-reader-other__hint {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}
</style>

<!-- ========================================================================
     markdown 排版（v-html 内容不参与 scoped，单独限定 .lr-reader-md 前缀）
     ===================================================================== -->
<style>
.lr-reader-md {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: var(--lr-space-6);
  font-size: var(--lr-fs-md, 15px);
  line-height: 1.75;
  color: var(--lr-text);
  word-break: break-word;
}

.lr-reader-md h1,
.lr-reader-md h2,
.lr-reader-md h3,
.lr-reader-md h4 {
  font-weight: var(--lr-fw-semibold, 600);
  line-height: var(--lr-lh-tight, 1.3);
  margin: 1.4em 0 0.6em;
}

.lr-reader-md h1 {
  font-size: 1.6em;
  padding-bottom: 0.3em;
  border-bottom: 1px solid var(--lr-border);
}

.lr-reader-md h2 {
  font-size: 1.35em;
}

.lr-reader-md h3 {
  font-size: 1.18em;
}

.lr-reader-md p {
  margin: 0.7em 0;
}

.lr-reader-md ul,
.lr-reader-md ol {
  margin: 0.7em 0;
  padding-left: 1.6em;
}

.lr-reader-md li {
  margin: 0.3em 0;
}

.lr-reader-md li > ul,
.lr-reader-md li > ol {
  margin: 0.2em 0;
}

.lr-reader-md blockquote {
  margin: 0.9em 0;
  padding: 0.2em 1em;
  border-left: 3px solid var(--lr-border-strong, var(--lr-border));
  color: var(--lr-text-secondary);
  background: var(--lr-bg-subtle, transparent);
}

.lr-reader-md code {
  font-family: var(--lr-font-mono, ui-monospace, monospace);
  font-size: 0.9em;
  background: var(--lr-bg-subtle, rgba(127, 127, 127, 0.12));
  padding: 0.15em 0.4em;
  border-radius: var(--lr-radius-sm, 4px);
}

.lr-reader-md pre {
  margin: 0.9em 0;
  padding: var(--lr-space-4);
  background: var(--lr-bg-code, rgba(127, 127, 127, 0.1));
  border-radius: var(--lr-radius-md, 8px);
  overflow: auto;
}

.lr-reader-md pre code {
  background: transparent;
  padding: 0;
  font-size: 0.88em;
  line-height: 1.6;
}

.lr-reader-md table {
  border-collapse: collapse;
  margin: 0.9em 0;
  width: 100%;
  display: block;
  overflow: auto;
  max-width: 100%;
}

.lr-reader-md th,
.lr-reader-md td {
  border: 1px solid var(--lr-border);
  padding: 0.4em 0.8em;
  font-size: 0.95em;
}

.lr-reader-md th {
  background: var(--lr-bg-subtle, rgba(127, 127, 127, 0.08));
  font-weight: 600;
}

.lr-reader-md img {
  max-width: 100%;
  border-radius: var(--lr-radius-md, 6px);
}

.lr-reader-md img.is-broken {
  opacity: 0.5;
  outline: 1px dashed var(--lr-danger);
}

.lr-reader-md a {
  color: var(--lr-primary);
  text-decoration: none;
}

.lr-reader-md a:hover {
  text-decoration: underline;
}

.lr-reader-md hr {
  border: none;
  border-top: 1px solid var(--lr-border);
  margin: 1.4em 0;
}
</style>
