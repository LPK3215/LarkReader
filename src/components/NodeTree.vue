<script setup lang="ts">
// ============================================================================
// NodeTree —— 知识库可勾选树
//
// 输入：api/types.ts 的 WikiNode 树（后端 get_wiki_tree 返回，整树传入）
// 输出：v-model 绑定选中的 node_token 列表
//
// 实现收敛在本组件内部（用 Naive UI 的 NTree），外部只认 WikiNode 与 token 数组。
// 超过 500 个节点自动切换虚拟滚动（仅渲染可视行），接口不变。
// ============================================================================

import { computed, h } from "vue";
import { NTree, type TreeOption } from "naive-ui";
import type { WikiNode, WikiNodeType } from "../api/types";
import AppIcon from "./AppIcon.vue";
import type { NodeRunState } from "../stores/task";

interface NodeOption extends TreeOption {
  key: string;
  title: string;
  objType: WikiNodeType;
  children?: NodeOption[];
}

const props = defineProps<{
  nodes: WikiNode[];
  selected: string[];
  nodeStates?: Record<string, NodeRunState>;
  disabled?: boolean;
}>();

const emit = defineEmits<{ "update:selected": [string[]] }>();

const TYPE_ICON: Record<WikiNodeType, string> = {
  doc: "doc",
  sheet: "sheet",
  bitable: "bitable",
  file: "paperclip",
  folder: "folder",
  other: "other",
};

const TYPE_TEXT: Record<WikiNodeType, string> = {
  doc: "文档",
  sheet: "表格",
  bitable: "多维表格",
  file: "附件",
  folder: "目录",
  other: "其他",
};

function toOption(node: WikiNode): NodeOption {
  return {
    key: node.node_token,
    title: node.title,
    objType: node.obj_type,
    isLeaf: !node.has_child && node.children.length === 0,
    children: node.children?.length ? node.children.map(toOption) : undefined,
  };
}

const treeData = computed<NodeOption[]>(() => props.nodes.map(toOption));

const checkedKeys = computed({
  get: () => props.selected,
  set: (value: Array<string | number>) => emit("update:selected", value.map(String)),
});

/** 默认展开前两层，避免大树一片折叠 */
const defaultExpanded = computed(() => {
  const keys: string[] = [];
  const walk = (nodes: WikiNode[], depth: number) => {
    nodes.forEach((node) => {
      if (depth < 2 && node.children.length) {
        keys.push(node.node_token);
        walk(node.children, depth + 1);
      }
    });
  };
  walk(props.nodes, 0);
  return keys;
});

const allKeys = computed(() => {
  const keys: string[] = [];
  const walk = (nodes: WikiNode[]) => {
    nodes.forEach((node) => {
      keys.push(node.node_token);
      walk(node.children);
    });
  };
  walk(props.nodes);
  return keys;
});

/**
 * 节点规模较大时启用虚拟滚动：只渲染可视区域的行。
 * 任务进行中 nodeStates 逐节点变化会触发重渲染，大树（数千行）下
 * 没有虚拟滚动会导致整棵树的 renderSuffix 全部重跑、界面明显卡顿。
 */
const largeTree = computed(() => allKeys.value.length > 500);
/** 虚拟滚动要求树自身具备确定高度，否则无法计算可视窗口 */
const treeVirtualStyle = computed(() =>
  largeTree.value ? { height: "100%" } : undefined
);

const allChecked = computed(
  () => allKeys.value.length > 0 && props.selected.length >= allKeys.value.length
);

function toggleAll() {
  emit("update:selected", allChecked.value ? [] : [...allKeys.value]);
}

function renderPrefix({ option }: { option: TreeOption }) {
  const node = option as NodeOption;
  return h("span", { class: `lr-tree__type lr-type--${node.objType}` }, [
    h(AppIcon, { name: TYPE_ICON[node.objType], size: 14 }),
  ]);
}

function renderSuffix({ option }: { option: TreeOption }) {
  const node = option as NodeOption;
  const state = props.nodeStates?.[node.key];
  const typeText = h("span", { class: "lr-tree__badge" }, TYPE_TEXT[node.objType]);

  if (!state) return typeText;

  const icon =
    state === "failed" || state === "skipped" ? "close-circle" : "check-circle";
  const cls =
    state === "failed" || state === "skipped"
      ? "lr-tree__state lr-tree__state--bad"
      : "lr-tree__state lr-tree__state--ok";

  return h("span", { class: "lr-tree__suffix" }, [
    typeText,
    h("span", { class: cls }, [h(AppIcon, { name: icon, size: 13 })]),
  ]);
}

function renderLabel({ option }: { option: TreeOption }) {
  const node = option as NodeOption;
  return h("span", { class: "lr-tree__label", title: node.title }, node.title);
}
</script>

<template>
  <div class="lr-tree">
    <div class="lr-tree__toolbar">
      <button class="lr-btn lr-btn--ghost lr-tree__all" @click="toggleAll">
        {{ allChecked ? "取消全选" : "全选" }}
      </button>
      <span class="lr-tree__count">已选 {{ selected.length }} / {{ allKeys.length }}</span>
    </div>

    <div class="lr-tree__scroll" :class="{ 'lr-tree__scroll--virtual': largeTree }">
      <NTree
        v-model:checked-keys="checkedKeys"
        :data="treeData"
        :default-expanded-keys="defaultExpanded"
        :disabled="disabled"
        :render-prefix="renderPrefix"
        :render-suffix="renderSuffix"
        :render-label="renderLabel"
        :virtual-scroll="largeTree"
        :style="treeVirtualStyle"
        checkable
        cascade
        block-line
        :selectable="false"
        class="lr-tree__widget"
      />
    </div>
  </div>
</template>

<style scoped>
.lr-tree {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.lr-tree__toolbar {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--lr-space-3);
  padding: var(--lr-space-2) var(--lr-space-4);
  border-bottom: 0.5px solid var(--lr-border);
}

.lr-tree__all {
  height: 24px;
  padding: 0 var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
}

.lr-tree__count {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-tree__scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: var(--lr-space-2) 0;
}

/* 虚拟滚动模式：外层不再滚动，由树内部虚拟列表接管，padding 归零避免裁剪首尾 */
.lr-tree__scroll--virtual {
  overflow: hidden;
  padding: 0;
}

.lr-tree__widget {
  font-size: var(--lr-fs-body);
}

.lr-tree__label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 以下作用于 render 函数产出的节点，需要 :deep 穿透 */
.lr-tree__scroll :deep(.lr-tree__type) {
  display: inline-flex;
  align-items: center;
  margin-right: 4px;
}

.lr-tree__scroll :deep(.lr-tree__badge) {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-tree__scroll :deep(.lr-tree__suffix) {
  display: inline-flex;
  align-items: center;
  gap: var(--lr-space-2);
}

.lr-tree__scroll :deep(.lr-tree__state--ok) {
  color: var(--lr-success);
}

.lr-tree__scroll :deep(.lr-tree__state--bad) {
  color: var(--lr-danger);
}
</style>
