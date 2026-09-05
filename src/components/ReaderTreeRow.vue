<script setup lang="ts">
// ============================================================================
// ReaderTreeRow —— 目录树的一行（自身递归渲染子级）
//
// 约定：
//   - 目录可展开/收起（惰性加载：第一次展开时才 list_reader_dir）
//   - `{stem}_images` 是导出的图片资源目录：弱化展示、不可展开
//   - .md 文档 emit select 供阅读；其他文件为附件，只展示不可读
// ============================================================================

import { computed, nextTick, ref, watch } from "vue";
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
    /**
     * 需要自动展开定位到的文档路径（任务历史「应用内阅读」直达用）。
     * 目录若位于该文档的祖先链上会自动展开，定位目标 md 行会滚动进可视区。
     */
    revealPath?: string | null;
  }>(),
  { depth: 0, activePath: null, revealPath: null }
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

/** 判断 child 是否等于或位于 ancestor 目录之下（兼容 / 与 \ 两种分隔符） */
function isPathUnder(child: string, ancestor: string): boolean {
  const c = child.replace(/\\/g, "/");
  const a = ancestor.replace(/\\/g, "/");
  if (c === a) return true;
  const prefix = a.endsWith("/") ? a : `${a}/`;
  return c.startsWith(prefix);
}

const isActive = computed(
  () => props.node.kind === "md" && props.node.path === props.activePath
);

/** 当前行是待定位文档本身（md） */
const isRevealTarget = computed(
  () => props.node.kind === "md" && !!props.revealPath && props.node.path === props.revealPath
);

/** 当前行是待定位文档的祖先目录（需要自动展开） */
const isRevealAncestor = computed(
  () =>
    props.node.kind === "dir" &&
    !props.node.imagesDir &&
    !!props.revealPath &&
    isPathUnder(props.revealPath, props.node.path)
);

/** 行元素：active / reveal 目标自动滚入可视区 */
const rowEl = ref<HTMLElement | null>(null);
watch(
  () => [isActive.value, isRevealTarget.value, rowEl.value] as const,
  ([active]) => {
    if ((active as boolean) && rowEl.value) {
      void nextTick(() => rowEl.value?.scrollIntoView({ block: "nearest" }));
    }
  },
  { immediate: true, flush: "post" }
);

/** 惰性加载子级；失败时把错误抛给上层展示。返回是否加载成功。 */
async function loadChildren(): Promise<boolean> {
  const node = props.node;
  if (node.imagesDir || node.loading || node.loaded) return node.loaded;
  node.loading = true;
  try {
    const entries = await listReaderDir(node.path);
    node.children = entries.map(entryToNode);
    node.loaded = true;
  } catch (err) {
    emit("error", `读取目录失败：${String(err)}`);
    node.loaded = false;
  } finally {
    node.loading = false;
  }
  return node.loaded;
}

async function toggleDir() {
  const node = props.node;
  if (node.kind !== "dir" || node.imagesDir || node.loading) return;
  const loaded = await loadChildren();
  if (loaded) node.expanded = !node.expanded;
}

/** 若当前目录是 reveal 目标祖先：加载并展开，让子行递归继续定位 */
watch(
  () => [props.revealPath, props.node.loaded] as const,
  async () => {
    if (!isRevealAncestor.value) return;
    const loaded = await loadChildren();
    if (loaded && !props.node.expanded) {
      props.node.expanded = true;
    }
  },
  { immediate: true }
);

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
      ref="rowEl"
      class="lr-treerow"
      :class="{
        'is-active': isActive,
        'is-reveal': isRevealTarget,
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
        :reveal-path="props.revealPath"
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
