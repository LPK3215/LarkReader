<script setup lang="ts">
// ============================================================================
// TaskPanel —— 下载任务进度面板（工作台右侧栏内）
//
// 输入：stores/task.ts 的 phase / 计数 / 当前文档 / 节点状态
// 展示：八阶段流程条、总进度、成功失败计数、当前文档、取消按钮
//
// 设计要点：失败不弹窗、不中断，只在计数里累加，明细留到结果卡统一看。
// ============================================================================

import { computed } from "vue";
import { PHASE_FLOW, PHASE_LABEL } from "../stores/task";
import type { TaskPhase } from "../api/types";
import AppIcon from "./AppIcon.vue";

const props = defineProps<{
  phase: TaskPhase;
  phaseLabel: string;
  done: number;
  total: number;
  successCount: number;
  failedCount: number;
  currentDoc: string | null;
  estimatedRemainingSeconds: number | null;
  cancelled?: boolean;
  finished?: boolean;
}>();

const emit = defineEmits<{ cancel: [] }>();

const phaseIndex = computed(() => PHASE_FLOW.indexOf(props.phase));

const remainingText = computed(() => {
  const sec = props.estimatedRemainingSeconds;
  if (sec === null) return null;
  if (sec <= 0) return "即将完成";
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return m > 0 ? `${m} 分 ${s} 秒` : `${s} 秒`;
});
</script>

<template>
  <div class="lr-taskpanel">
    <div class="lr-taskpanel__head">
      <AppIcon v-if="!finished" name="spinner" :size="14" class="lr-taskpanel__spinner lr-icon-spin" />
      <AppIcon v-else-if="failedCount" name="alert-circle" :size="14" class="lr-taskpanel__warn" />
      <AppIcon v-else name="check-circle" :size="14" class="lr-taskpanel__ok" />
      <span class="lr-taskpanel__phase">{{ phaseLabel }}</span>
      <span v-if="cancelled" class="lr-badge lr-badge--warning">已取消</span>
    </div>

    <!-- 八阶段流程条 -->
    <ol class="lr-steps">
      <li
        v-for="(p, i) in PHASE_FLOW"
        :key="p"
        class="lr-steps__item"
        :class="{
          'is-done': finished || i < phaseIndex,
          'is-current': !finished && i === phaseIndex,
        }"
      >
        <span class="lr-steps__dot" />
        <span class="lr-steps__text">{{ PHASE_LABEL[p] }}</span>
      </li>
    </ol>

    <div class="lr-taskpanel__bar">
      <div class="lr-taskpanel__track">
        <div
          class="lr-taskpanel__fill"
          :style="{ width: total ? `${Math.round((done / total) * 100)}%` : '0%' }"
        />
      </div>
      <div class="lr-taskpanel__nums">
        <span>{{ done }} / {{ total }}</span>
        <span v-if="remainingText" class="lr-taskpanel__eta">剩余 {{ remainingText }}</span>
      </div>
    </div>

    <div class="lr-taskpanel__stats">
      <span class="lr-taskpanel__stat">
        <AppIcon name="check" :size="12" />
        成功 {{ successCount }}
      </span>
      <span v-if="failedCount" class="lr-taskpanel__stat lr-taskpanel__stat--bad">
        <AppIcon name="close" :size="12" />
        失败 {{ failedCount }}
      </span>
    </div>

    <div v-if="currentDoc" class="lr-taskpanel__current">
      <span class="lr-taskpanel__k">正在处理</span>
      <span class="lr-taskpanel__v lr-mono lr-selectable">{{ currentDoc }}</span>
    </div>

    <button
      v-if="!finished"
      class="lr-btn lr-btn--danger lr-btn--block"
      :disabled="cancelled"
      @click="emit('cancel')"
    >
      {{ cancelled ? "正在取消…" : "取消任务" }}
    </button>
  </div>
</template>

<style scoped>
.lr-taskpanel {
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-4);
}

.lr-taskpanel__head {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
}

.lr-taskpanel__spinner {
  color: var(--lr-primary);
}

.lr-taskpanel__ok {
  color: var(--lr-success);
}

.lr-taskpanel__warn {
  color: var(--lr-warning);
}

.lr-taskpanel__phase {
  font-size: var(--lr-fs-section);
  font-weight: var(--lr-fw-medium);
}

/* ---- 阶段流程条 ---- */

.lr-steps {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-2);
}

.lr-steps__item {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-steps__dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--lr-border-hover);
  flex: none;
}

.lr-steps__item.is-done {
  color: var(--lr-text-secondary);
}

.lr-steps__item.is-done .lr-steps__dot {
  background: var(--lr-success);
}

.lr-steps__item.is-current {
  color: var(--lr-primary);
  font-weight: var(--lr-fw-medium);
}

.lr-steps__item.is-current .lr-steps__dot {
  background: var(--lr-primary);
  box-shadow: 0 0 0 3px var(--lr-primary-soft);
}

/* ---- 进度条 ---- */

.lr-taskpanel__track {
  height: 6px;
  border-radius: 3px;
  background: var(--lr-bg-active);
  overflow: hidden;
}

.lr-taskpanel__fill {
  height: 100%;
  border-radius: 3px;
  background: var(--lr-primary);
  transition: width 0.3s ease-out;
}

.lr-taskpanel__nums {
  display: flex;
  justify-content: space-between;
  margin-top: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
}

.lr-taskpanel__eta {
  color: var(--lr-text-tertiary);
}

/* ---- 计数 ---- */

.lr-taskpanel__stats {
  display: flex;
  gap: var(--lr-space-4);
}

.lr-taskpanel__stat {
  display: inline-flex;
  align-items: center;
  gap: var(--lr-space-1);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-success);
}

.lr-taskpanel__stat--bad {
  color: var(--lr-danger);
}

/* ---- 当前文档 ---- */

.lr-taskpanel__current {
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-1);
  padding: var(--lr-space-2) var(--lr-space-3);
  border-radius: var(--lr-radius-md);
  background: var(--lr-bg-subtle);
}

.lr-taskpanel__k {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-taskpanel__v {
  color: var(--lr-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
