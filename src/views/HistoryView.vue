<script setup lang="ts">
// ============================================================================
// HistoryView —— 任务历史页
//
// 数据：stores/history.ts records（list_task_history 结果）
// 操作：刷新、打开产物目录、删除单条
//
// 真机专享：所有动作走 IPC；不再保留浏览器假数据兜底。
// ============================================================================

import { onMounted } from "vue";
import { useHistoryStore } from "../stores/history";
import type { TaskStatus } from "../api/types";
import AppIcon from "../components/AppIcon.vue";

const history = useHistoryStore();

const STATUS_TEXT: Record<TaskStatus, string> = {
  pending: "等待中",
  running: "进行中",
  completed: "已完成",
  failed: "失败",
  cancelled: "已取消",
};

const STATUS_CLASS: Record<TaskStatus, string> = {
  pending: "lr-badge",
  running: "lr-badge--info",
  completed: "lr-badge--success",
  failed: "lr-badge--danger",
  cancelled: "lr-badge--warning",
};

function formatTime(iso: string | null) {
  if (!iso) return "—";
  const d = new Date(iso);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function onOpenDir(record: { result: { output_root: string } | null }) {
  if (record.result?.output_root) {
    history.openDir(record.result.output_root);
  }
}

async function onRefresh() {
  await history.refresh();
}

onMounted(async () => {
  await history.load();
});
</script>

<template>
  <div class="lr-page">
    <header class="lr-page__head">
      <div class="lr-history__head">
        <div>
          <h1 class="lr-page__title">任务历史</h1>
          <p class="lr-page__desc">保留最近 24 小时、最多 100 条，旧的自动淘汰</p>
        </div>
        <button
          class="lr-btn lr-btn--secondary"
          :disabled="history.loading"
          @click="onRefresh"
        >
          <AppIcon name="refresh" :size="14" />
          {{ history.loading ? "刷新中…" : "刷新" }}
        </button>
      </div>
    </header>

    <div class="lr-page__body">
      <p v-if="history.lastError" class="lr-history__errbar">
        <AppIcon name="alert-circle" :size="14" />
        {{ history.lastError }}
      </p>

      <div
        v-if="history.records.length === 0"
        class="lr-empty"
      >
        <AppIcon name="history" :size="28" />
        <span>{{ history.loading ? "加载中…" : "还没有导出记录" }}</span>
      </div>

      <section v-else class="lr-card lr-history">
        <ul class="lr-history__list">
          <li
            v-for="record in history.records"
            :key="record.task_id"
            class="lr-history__row"
          >
            <div class="lr-history__main">
              <div class="lr-history__line">
                <span class="lr-badge" :class="STATUS_CLASS[record.progress.status]">
                  {{ STATUS_TEXT[record.progress.status] }}
                </span>
                <span class="lr-history__name lr-selectable">
                  {{ record.result?.wiki_name || "未命名的导出" }}
                </span>
              </div>

              <div class="lr-history__meta">
                <span>{{ formatTime(record.progress.finished_at) }}</span>
                <span v-if="record.progress.total">
                  {{ record.progress.done }} / {{ record.progress.total }} 项
                </span>
                <span v-if="record.progress.success_count" class="lr-history__ok">
                  成功 {{ record.progress.success_count }}
                </span>
                <span v-if="record.progress.failed_count" class="lr-history__bad">
                  失败 {{ record.progress.failed_count }}
                </span>
                <span v-if="record.progress.elapsed_seconds">
                  用时 {{ record.progress.elapsed_seconds }} 秒
                </span>
              </div>

              <p v-if="record.error" class="lr-history__error lr-selectable">
                {{ record.error }}
              </p>

              <code
                v-if="record.result?.output_root"
                class="lr-history__path lr-selectable"
              >
                {{ record.result.output_root }}
              </code>
            </div>

            <div class="lr-history__ops">
              <button
                v-if="record.result?.output_root"
                class="lr-btn lr-btn--secondary"
                title="打开产物目录"
                @click="onOpenDir(record)"
              >
                <AppIcon name="folder-open" :size="14" />
                打开
              </button>
              <button
                class="lr-btn lr-btn--ghost"
                title="删除这条记录"
                @click="history.remove(record.task_id)"
              >
                <AppIcon name="trash" :size="14" />
              </button>
            </div>
          </li>
        </ul>
      </section>
    </div>
  </div>
</template>

<style scoped>
.lr-history__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--lr-space-4);
}

.lr-history__errbar {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  padding: var(--lr-space-3) var(--lr-space-4);
  margin-bottom: var(--lr-space-3);
  border-radius: var(--lr-radius-md);
  background: var(--lr-danger-soft);
  border: 0.5px solid var(--lr-danger-border);
  color: var(--lr-danger);
  font-size: var(--lr-fs-secondary);
}

.lr-history {
  height: 100%;
  overflow: auto;
}

.lr-history__list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.lr-history__row {
  display: flex;
  align-items: flex-start;
  gap: var(--lr-space-4);
  padding: var(--lr-space-3) var(--lr-space-4);
  border-bottom: 0.5px solid var(--lr-border);
  transition: background 0.15s;
}

.lr-history__row:last-child {
  border-bottom: none;
}

.lr-history__row:hover {
  background: var(--lr-bg-subtle);
}

.lr-history__main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-1);
}

.lr-history__line {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
}

.lr-history__name {
  font-size: var(--lr-fs-body);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.lr-history__meta {
  display: flex;
  flex-wrap: wrap;
  gap: var(--lr-space-3);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-history__ok {
  color: var(--lr-success);
}

.lr-history__bad {
  color: var(--lr-danger);
}

.lr-history__error {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-danger);
}

.lr-history__path {
  font-size: var(--lr-fs-mono);
  color: var(--lr-text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.lr-history__ops {
  flex: none;
  display: flex;
  gap: var(--lr-space-2);
}
</style>