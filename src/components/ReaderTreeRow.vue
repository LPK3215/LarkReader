<script setup lang="ts">
// ============================================================================
// ReaderTreeRow —— 目录树的一行（自身递归渲染子级）
//
// 约定：
//   - 目录可展开/收起（惰性加载：第一次展开时才 list_reader_dir）
//   - `{stem}_images` 是导出的图片资源目录：弱化展示、不可展开
//   - .md 文档 emit select 供阅读；其他文件为附件，只展示不可读
// ============================================================================

import AppIcon from "./AppIcon.vue";
import { listReaderDir } from "../api/reader";
import type { ReaderEntry, ReaderEntryKind } from "../api/types";

export interface ReaderTreeNode {
  name: string;
  path: string;
  kind: ReaderEntryKind;
  /** 图片资源目录（{stem}_images）：弱化展示、不可展开 */
  imagesDir: boolean;
  sizeBytes: number | null;
  expanded: boolean;
  loading: boolean;
  loaded: boolean;
  children: ReaderTreeNode[];
}

defineOptions({ name: "ReaderTreeRow" });

const props = withDefaults(
  defineProps<{
    node: ReaderTreeNode;
    depth?: number;
    activePath?: string | null;
  }>(),
  { depth: 0, activePath: null }
);

const emit = defineEmits<{
  select: [path: string];
  error: [message: string];
}>();

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

async function toggleDir() {
  const node = props.node;
  if (node.imagesDir || node.loading) return;
  if (!node.loaded) {
    node.loading = true;
    try {
      const entries = await listReaderDir(node.path);
      node.children = entries.map(entryToNode);
      node.loaded = true;
    } catch (err) {
      emit("error", `读取目录失败：${String(err)}`);
      return;
    } finally {
      node.loading = false;
    }
  }
  node.expanded = !node.expanded;
}

function onLineClick() {
  const node = props.node;
  if (node.kind === "dir") {
    void toggleDir();
  } else if (node.kind === "md") {
    emit("select", node.path);
  }
  // other（附件）与 images 目录不可点：仅展示
}

function sizeText(bytes: number | null): string {
  if (bytes == null) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
</script>

<template>
  <div>
    <div
      class="lr-treerow"
      :class="{
        'is-active': node.kind === 'md' && node.path === props.activePath,
        'is-clickable': node.kind === 'dir' ? !node.imagesDir : node.kind === 'md',
        'is-dim': node.imagesDir || node.kind === 'other',
      }"
      :style="{ paddingLeft: `${8 + (props.depth ?? 0) * 16}px` }"
      :title="node.name"
      @click="onLineClick"
    >
      <span class="lr-treerow__twist">
        <AppIcon
          v-if="node.kind === 'dir'"
          :name="node.expanded && node.loaded ? 'chevronDown' : 'chevronRight'"
          :size="12"
        />
        <AppIcon v-else-if="node.loading" name="spinner" :size="11" />
      </span>

      <AppIcon
        :name="
          node.imagesDir
            ? 'folder'
            : node.kind === 'dir'
              ? node.expanded && node.loaded
                ? 'folder-open'
                : 'folder'
              : node.kind === 'md'
                ? 'doc'
                : 'paperclip'
        "
        :size="15"
        class="lr-treerow__icon"
        :class="{ 'is-dim': node.kind === 'other' }"
      />

      <span class="lr-treerow__name">{{ node.name }}</span>

      <span v-if="node.sizeBytes != null" class="lr-treerow__size">
        {{ sizeText(node.sizeBytes) }}
      </span>
    </div>

    <template v-if="node.kind === 'dir' && node.expanded && node.loaded">
      <ReaderTreeRow
        v-for="child in node.children"
        :key="child.path"
        :node="child"
        :depth="(props.depth ?? 0) + 1"
        :active-path="props.activePath"
        @select="(p) => emit('select', p)"
        @error="(m) => emit('error', m)"
      />
    </template>
  </div>
</template>

<style scoped>
.lr-treerow {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  padding-top: 3px;
  padding-bottom: 3px;
  border-radius: var(--lr-radius-sm);
  color: var(--lr-text);
  cursor: default;
  user-select: none;
  white-space: nowrap;
}

.lr-treerow:hover {
  background: var(--lr-bg-hover);
}

.lr-treerow.is-clickable {
  cursor: pointer;
}

.lr-treerow.is-active {
  background: var(--lr-primary-soft);
  color: var(--lr-primary);
}

.lr-treerow.is-dim {
  color: var(--lr-text-tertiary);
}

.lr-treerow__twist {
  width: 12px;
  flex: none;
  display: inline-flex;
  justify-content: center;
}

.lr-treerow__icon {
  flex: none;
}

.lr-treerow__icon.is-dim {
  opacity: 0.45;
}

.lr-treerow__name {
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
}

.lr-treerow__size {
  margin-left: auto;
  flex: none;
  padding-right: 4px;
  color: var(--lr-text-disabled);
  font-size: var(--lr-fs-small);
}
</style>
