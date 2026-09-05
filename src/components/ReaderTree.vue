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
  }>(),
  { activePath: null }
);

const emit = defineEmits<{
  select: [path: string];
  error: [message: string];
}>();

const children = ref<ReaderTreeNode[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);

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

function onError(message: string) {
  error.value = message;
  emit("error", message);
}
</script>

<template>
  <div class="lr-tree">
    <div v-if="loading" class="lr-tree__hint">
      <AppIcon name="spinner" :size="12" />
      加载目录中…
    </div>

    <div v-else-if="error" class="lr-tree__error">
      <AppIcon name="alert-circle" :size="12" />
      {{ error }}
    </div>

    <div v-else-if="children.length === 0" class="lr-tree__hint">（空目录）</div>

    <template v-else>
      <ReaderTreeRow
        v-for="child in children"
        :key="child.path"
        :node="child"
        :active-path="props.activePath"
        @select="onSelect"
        @error="onError"
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
.lr-tree__error {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  padding: var(--lr-space-3);
  color: var(--lr-text-tertiary);
  font-size: var(--lr-fs-secondary);
}

.lr-tree__error {
  color: var(--lr-danger);
}
</style>
