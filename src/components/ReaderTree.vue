<script setup lang="ts">
// ============================================================================
// ReaderTree —— 本地阅读页的目录导航树（根层加载，子级由 Row 惰性展开）
//
// 数据源：list_reader_dir 一次一层；文件树保留导出时的知识库目录层级。
// 打开 .md 通过 emit('select') 交给阅读页渲染。
// ============================================================================

import { ref, watch } from "vue";
import AppIcon from "./AppIcon.vue";
import ReaderTreeRow, { type ReaderTreeNode } from "./ReaderTreeRow.vue";
import { listReaderDir } from "../api/reader";
import type { ReaderEntry } from "../api/types";

const props = withDefaults(
  defineProps<{
    rootPath: string;
    activePath?: string | null;
    /** 需要自动展开定位的文档路径（由父页面透传，见 ReaderTreeRow） */
    revealPath?: string | null;
  }>(),
  { activePath: null, revealPath: null }
);

const emit = defineEmits<{
  select: [path: string];
}>();

const children = ref<ReaderTreeNode[]>([]);
const loading = ref(false);
/** 根目录整层加载失败的提示（可整块重试） */
const error = ref<string | null>(null);
/** 子级惰性展开失败的非阻塞通知（只影响展开动作，不清空已加载的树） */
const notice = ref<string | null>(null);

function entryToNode(entry: ReaderEntry): ReaderTreeNode {
  const imagesDir = entry.kind === "dir" && entry.name.endsWith("_images");
  return {
    name: entry.name,
    path: entry.path,
    kind: entry.kind,
    imagesDir,
    sizeBytes: entry.size_bytes,
    expanded: false,
    loading: false,
    loaded: false,
    children: [],
  };
}

watch(
  () => props.rootPath,
  (path) => {
    if (path) void reload(path);
  },
  { immediate: true }
);

async function reload(path: string) {
  loading.value = true;
  error.value = null;
  // 切换到新根目录时清掉旧目录残留的展开失败提示
  notice.value = null;
  try {
    children.value = (await listReaderDir(path)).map(entryToNode);
  } catch (err) {
    error.value = String(err);
    children.value = [];
  } finally {
    loading.value = false;
  }
}

function onSelect(path: string) {
  emit("select", path);
}

/** 某个子目录展开失败：只在树顶提示，让用户重试/继续浏览其它目录 */
function onChildError(message: string) {
  notice.value = message;
}
</script>

<template>
  <div class="lr-tree">
    <!-- 子目录展开失败：非阻塞提示条（不影响已展开的内容） -->
    <div v-if="notice" class="lr-tree__notice">
      <AppIcon name="alert-circle" :size="12" />
      <span class="lr-tree__errmsg">{{ notice }}</span>
      <button
        class="lr-btn lr-btn--ghost lr-tree__retry"
        title="关闭提示（可再次点击该目录重试）"
        @click="notice = null"
      >
        知道了
      </button>
    </div>

    <div v-if="loading" class="lr-tree__hint">
      <AppIcon name="spinner" :size="12" class="lr-icon-spin" />
      加载目录中…
    </div>

    <div v-else-if="error" class="lr-tree__error">
      <AppIcon name="alert-circle" :size="12" />
      <span class="lr-tree__errmsg">{{ error }}</span>
      <button class="lr-btn lr-btn--ghost lr-tree__retry" @click="reload(props.rootPath)">
        重试
      </button>
    </div>

    <div v-else-if="children.length === 0" class="lr-tree__hint">（空目录）</div>

    <template v-else>
      <ReaderTreeRow
        v-for="child in children"
        :key="child.path"
        :node="child"
        :active-path="props.activePath"
        :reveal-path="props.revealPath"
        @select="onSelect"
        @error="onChildError"
      />
    </template>
  </div>
</template>

<style scoped>
.lr-tree {
  font-size: var(--lr-fs-secondary);
  min-height: 100%;
}

.lr-tree__hint,
.lr-tree__error,
.lr-tree__notice {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  padding: var(--lr-space-2) var(--lr-space-3);
  color: var(--lr-text-tertiary);
  font-size: var(--lr-fs-secondary);
}

.lr-tree__error {
  color: var(--lr-danger);
  gap: var(--lr-space-2);
  flex-wrap: wrap;
}

.lr-tree__notice {
  color: var(--lr-warning);
  border-bottom: 0.5px solid var(--lr-border);
  background: var(--lr-bg-subtle);
}

.lr-tree__notice .lr-tree__retry {
  flex: none;
  height: 22px;
  padding: 0 var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-warning);
  border-color: currentColor;
}

.lr-tree__errmsg {
  flex: 1;
  min-width: 0;
  word-break: break-word;
}

.lr-tree__retry {
  flex: none;
  height: 24px;
  padding: 0 var(--lr-space-3);
  font-size: var(--lr-fs-secondary);
}
</style>
