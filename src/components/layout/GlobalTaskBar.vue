<script setup lang="ts">
// 全局任务条（40px）：跨页面保留，切到历史或设置页也能看到进度并随时取消。
// 数据来自 stores/task.ts 的 Progress（本轮为静态视觉骨架，先用 props 驱动）。

import AppIcon from "../AppIcon.vue";

withDefaults(
  defineProps<{
    phaseLabel?: string;
    done?: number;
    total?: number;
    currentDoc?: string | null;
    remaining?: string | null;
    successCount?: number;
    failedCount?: number;
    cancelled?: boolean;
    /** 是否已进入终态（完成/被取消都算），终态后不再显示取消、进度换结果图标 */
    finished?: boolean;
  }>(),
  {
    phaseLabel: "正在导出文档",
    done: 0,
    total: 0,
    currentDoc: null,
    remaining: null,
    successCount: 0,
    failedCount: 0,
    cancelled: false,
    finished: false,
  }
);

const emit = defineEmits<{ cancel: []; minimize: []; detail: [] }>();
</script>

<template>
  <div class="lr-taskbar">
    <!-- 运行中：转圈；终态：成功/有失败/已取消分别给出对应图标 -->
    <AppIcon
      v-if="finished && !cancelled && failedCount"
      name="alert-circle"
      :size="14"
      class="lr-taskbar__done lr-taskbar__done--warn"
    />
    <AppIcon
      v-else-if="finished && !cancelled"
      name="check-circle"
      :size="14"
      class="lr-taskbar__done lr-taskbar__done--ok"
    />
    <AppIcon v-else-if="finished" name="close-circle" :size="14" class="lr-taskbar__done" />
    <AppIcon v-else name="spinner" :size="14" class="lr-taskbar__spinner lr-icon-spin" />

    <div class="lr-taskbar__main">
      <div class="lr-taskbar__line">
        <span class="lr-taskbar__phase">{{ phaseLabel }}</span>
        <span class="lr-taskbar__count">
          {{ done }} / {{ total || "—" }}
        </span>
        <span v-if="successCount" class="lr-taskbar__ok">成功 {{ successCount }}</span>
        <span v-if="failedCount" class="lr-taskbar__bad">失败 {{ failedCount }}</span>
        <span v-if="remaining" class="lr-taskbar__eta">剩余 {{ remaining }}</span>
      </div>

      <div class="lr-taskbar__track">
        <div
          class="lr-taskbar__fill"
          :class="{
            'is-done': finished && !cancelled,
            'is-failed': finished && !cancelled && failedCount,
            'is-cancelled': finished && cancelled,
          }"
          :style="{ width: total ? `${Math.round((done / total) * 100)}%` : '0%' }"
        />
      </div>
    </div>

    <span v-if="currentDoc" class="lr-taskbar__doc lr-mono lr-selectable" :title="currentDoc">
      {{ currentDoc }}
    </span>

    <button class="lr-taskbar__btn" @click="emit('detail')">明细</button>
    <button class="lr-taskbar__btn" @click="emit('minimize')">收起</button>
    <button
      v-if="!finished"
      class="lr-taskbar__btn lr-taskbar__btn--danger"
      :disabled="cancelled"
      @click="emit('cancel')"
    >
      {{ cancelled ? "取消中…" : "取消" }}
    </button>
  </div>
</template>

<style scoped>
.lr-taskbar {
  height: var(--lr-taskbar-h);
  display: flex;
  align-items: center;
  gap: var(--lr-space-3);
  padding: 0 var(--lr-space-4);
  background: var(--lr-primary-soft);
  border-top: 0.5px solid var(--lr-primary-border);
}

.lr-taskbar__spinner {
  color: var(--lr-primary);
}

.lr-taskbar__done {
  color: var(--lr-text-tertiary);
}

.lr-taskbar__done--ok {
  color: var(--lr-success);
}

.lr-taskbar__done--warn {
  color: var(--lr-warning);
}

.lr-taskbar__main {
  width: 260px;
  flex: none;
}

.lr-taskbar__line {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  line-height: var(--lr-lh-tight);
}

.lr-taskbar__phase {
  color: var(--lr-text);
  font-weight: var(--lr-fw-medium);
}

.lr-taskbar__count {
  color: var(--lr-text-secondary);
}

.lr-taskbar__ok {
  color: var(--lr-success);
}

.lr-taskbar__bad {
  color: var(--lr-danger);
}

.lr-taskbar__eta {
  color: var(--lr-text-tertiary);
}

.lr-taskbar__track {
  margin-top: var(--lr-space-1);
  height: 3px;
  border-radius: 1.5px;
  background: var(--lr-border);
  overflow: hidden;
}

.lr-taskbar__fill {
  height: 100%;
  border-radius: 1.5px;
  background: var(--lr-primary);
  transition: width 0.3s ease-out, background 0.3s ease-out;
}

.lr-taskbar__fill.is-done {
  background: var(--lr-success);
}

.lr-taskbar__fill.is-failed {
  background: var(--lr-warning);
}

.lr-taskbar__fill.is-cancelled {
  background: var(--lr-text-tertiary);
}

.lr-taskbar__doc {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--lr-text-secondary);
}

.lr-taskbar__btn {
  height: 24px;
  padding: 0 10px;
  flex: none;
  border: 0.5px solid var(--lr-border-hover);
  border-radius: var(--lr-radius-md);
  background: transparent;
  color: var(--lr-text-secondary);
  font-size: var(--lr-fs-secondary);
  transition: background 0.15s, color 0.15s, border-color 0.15s;
}

.lr-taskbar__btn:hover:not(:disabled) {
  background: var(--lr-bg-surface);
  color: var(--lr-text);
}

.lr-taskbar__btn--danger:hover:not(:disabled) {
  color: var(--lr-danger);
  border-color: var(--lr-danger-border);
}

.lr-taskbar__btn:disabled {
  color: var(--lr-text-disabled);
  cursor: not-allowed;
}
</style>
