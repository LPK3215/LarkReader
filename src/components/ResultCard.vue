<script setup lang="ts">
// ============================================================================
// ResultCard —— 任务完成结果卡
//
// 输入：WikiExtractResult 的 items（统一明细，含 status 与产出路径）
// 展示：成功/部分/失败/跳过汇总、产物根路径 + 打开目录、可展开的失败明细
//
// 与 HistoryView、WorkspaceView 共用，因此所有数据走 props，不直接读 store。
// ============================================================================

import { computed, ref } from "vue";
import type { ExportItemResult, ExportItemStatus } from "../api/types";
import AppIcon from "./AppIcon.vue";

const props = withDefaults(
  defineProps<{
    wikiName?: string;
    outputRoot?: string;
    items?: ExportItemResult[];
    cancelled?: boolean;
    /** 是否显示右上角关闭按钮（由宿主场景决定，如历史记录弹层） */
    closable?: boolean;
  }>(),
  { wikiName: "", outputRoot: "", items: () => [], cancelled: false, closable: false }
);

const emit = defineEmits<{ openDir: []; again: []; close: [] }>();

const STATUS_TEXT: Record<ExportItemStatus, string> = {
  success: "成功",
  partial: "部分成功",
  failed: "失败",
  skipped: "跳过",
};

const STATUS_CLASS: Record<ExportItemStatus, string> = {
  success: "lr-badge--success",
  partial: "lr-badge--warning",
  failed: "lr-badge--danger",
  skipped: "lr-badge",
};

const summary = computed(() => {
  const counts: Record<ExportItemStatus, number> = {
    success: 0,
    partial: 0,
    failed: 0,
    skipped: 0,
  };
  props.items.forEach((item) => {
    counts[item.status] += 1;
  });
  return counts;
});

const problemItems = computed(() =>
  props.items.filter((item) => item.status !== "success")
);

const showProblems = ref(false);
</script>

<template>
  <section class="lr-result">
    <header class="lr-result__head">
      <div>
        <h3 class="lr-result__title">
          <AppIcon
            v-if="problemItems.length"
            name="alert-circle"
            :size="15"
            class="lr-result__warn"
          />
          <AppIcon v-else name="check-circle" :size="15" class="lr-result__ok" />
          {{ cancelled ? "任务已取消" : "导出完成" }}
        </h3>
        <p v-if="wikiName" class="lr-result__sub lr-selectable">{{ wikiName }}</p>
      </div>
      <button v-if="props.closable" class="lr-iconbtn" title="关闭" @click="emit('close')">
        <AppIcon name="close" :size="14" />
      </button>
    </header>

    <div class="lr-result__stats">
      <span class="lr-badge lr-badge--success">成功 {{ summary.success }}</span>
      <span v-if="summary.partial" class="lr-badge lr-badge--warning">
        部分成功 {{ summary.partial }}
      </span>
      <span v-if="summary.failed" class="lr-badge lr-badge--danger">
        失败 {{ summary.failed }}
      </span>
      <span v-if="summary.skipped" class="lr-badge">跳过 {{ summary.skipped }}</span>
    </div>

    <div v-if="outputRoot" class="lr-result__path">
      <span class="lr-result__k">产物目录</span>
      <code class="lr-result__v lr-selectable">{{ outputRoot }}</code>
    </div>

    <div class="lr-result__actions">
      <button class="lr-btn lr-btn--primary" @click="emit('openDir')">
        <AppIcon name="folder-open" :size="14" />
        打开目录
      </button>
      <button class="lr-btn lr-btn--secondary" @click="emit('again')">
        <AppIcon name="refresh" :size="14" />
        重新选择
      </button>
    </div>

    <div v-if="problemItems.length" class="lr-result__problems">
      <button class="lr-result__toggle" @click="showProblems = !showProblems">
        <AppIcon :name="showProblems ? 'chevronDown' : 'chevronRight'" :size="12" />
        查看 {{ problemItems.length }} 项异常明细
      </button>

      <ul v-if="showProblems" class="lr-result__list">
        <li v-for="item in problemItems" :key="item.node_token ?? item.title" class="lr-result__row">
          <span class="lr-badge" :class="STATUS_CLASS[item.status]">
            {{ STATUS_TEXT[item.status] }}
          </span>
          <div class="lr-result__rowmain">
            <span class="lr-result__rowtitle lr-selectable" :title="item.title">
              {{ item.title }}
            </span>
            <span v-if="item.message" class="lr-result__msg lr-selectable">
              {{ item.message }}
            </span>
          </div>
        </li>
      </ul>
    </div>
  </section>
</template>

<style scoped>
.lr-result {
  background: var(--lr-bg-surface);
  border: 0.5px solid var(--lr-border);
  border-radius: var(--lr-radius-lg);
  padding: var(--lr-space-4);
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-3);
}

.lr-result__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--lr-space-3);
}

.lr-result__title {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-section);
  font-weight: var(--lr-fw-medium);
}

.lr-result__ok {
  color: var(--lr-success);
}

.lr-result__warn {
  color: var(--lr-warning);
}

.lr-result__sub {
  margin-top: var(--lr-space-1);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
}

.lr-iconbtn {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: var(--lr-radius-md);
  background: transparent;
  color: var(--lr-text-tertiary);
}

.lr-iconbtn:hover {
  background: var(--lr-bg-hover);
  color: var(--lr-text);
}

.lr-result__stats {
  display: flex;
  flex-wrap: wrap;
  gap: var(--lr-space-2);
}

.lr-result__path {
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-1);
  padding: var(--lr-space-3);
  border-radius: var(--lr-radius-md);
  background: var(--lr-bg-subtle);
}

.lr-result__k {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-result__v {
  color: var(--lr-text-secondary);
  word-break: break-all;
}

.lr-result__actions {
  display: flex;
  gap: var(--lr-space-2);
}

.lr-result__problems {
  border-top: 0.5px solid var(--lr-border);
  padding-top: var(--lr-space-3);
}

.lr-result__toggle {
  display: inline-flex;
  align-items: center;
  gap: var(--lr-space-1);
  border: none;
  background: transparent;
  padding: 0;
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
}

.lr-result__toggle:hover {
  color: var(--lr-text);
}

.lr-result__list {
  list-style: none;
  margin: var(--lr-space-3) 0 0;
  padding: 0;
  max-height: 180px;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-2);
}

.lr-result__row {
  display: flex;
  align-items: flex-start;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
}

.lr-result__row > .lr-badge {
  flex: none;
  margin-top: 1px;
}

.lr-result__rowmain {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  flex: 1;
}

.lr-result__rowtitle {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.lr-result__msg {
  line-height: 1.5;
  word-break: break-word;
  white-space: pre-wrap;
  color: var(--lr-text-tertiary);
}
</style>
