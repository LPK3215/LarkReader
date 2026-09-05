<script setup lang="ts">
// ============================================================================
// WorkspaceView —— 主工作台
//
// 三态单页流（不分步、不切页）：
//   empty   只等一个链接，居中卡片
//   tree    输入条收起为一行 + 左树勾选 + 右侧栏（输出目录 / 选中摘要 / 开始）
//   running 树不销毁，逐节点打勾打叉；右侧栏换成 TaskPanel
//   done    右侧栏换成 ResultCard，可「重新选择」回到 tree
//
// 浏览器 dev 模式（isTauri() === false）走 mock；空态底部展示"试一下 demo 树"按钮。
// ============================================================================

import { computed, ref } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import { useTaskStore } from "../stores/task";
import { useSettingsStore } from "../stores/settings";
import NodeTree from "../components/NodeTree.vue";
import TaskPanel from "../components/TaskPanel.vue";
import ResultCard from "../components/ResultCard.vue";
import DirPicker from "../components/DirPicker.vue";
import AppIcon from "../components/AppIcon.vue";

const task = useTaskStore();
const settings = useSettingsStore();

const inputUrl = ref("");
const showDemo = computed(() => !isTauri());

const treeNodes = computed(() => (task.tree ? [task.tree] : []));

const sideTitle = computed(() => {
  if (task.finished) return "导出结果";
  if (task.running) return "任务进度";
  return "导出设置";
});

function onScan() {
  task.scan(inputUrl.value);
}

function onStart() {
  task.start();
}

function tryDemo() {
  inputUrl.value = "https://example.feishu.cn/wiki/DEMO";
  onScan();
}
</script>

<template>
  <div class="lr-work lr-page">
    <!-- 空态 -->
    <div v-if="task.stage === 'empty'" class="lr-work__empty">
      <div class="lr-work__emptycard">
        <span class="lr-work__emptylogo"><AppIcon name="link" :size="22" /></span>
        <h2 class="lr-work__emptytitle">导出飞书知识库</h2>
        <p class="lr-work__emptydesc">
          粘贴知识库链接，先只扫描目录结构，勾选需要的节点再下载
        </p>

        <div class="lr-work__emptyform">
          <input
            v-model="inputUrl"
            class="lr-input"
            placeholder="https://xxx.feishu.cn/wiki/..."
            @keyup.enter="onScan"
          />
          <button class="lr-btn lr-btn--primary lr-btn--lg" @click="onScan">扫描结构</button>
        </div>

        <p class="lr-work__emptynote">
          扫描阶段只读取目录树，不下载正文、不写入磁盘
        </p>

        <button v-if="showDemo" class="lr-btn lr-btn--ghost lr-work__demobtn" @click="tryDemo">
          <AppIcon name="play" :size="12" />
          试一下 demo 树（仅浏览器预览模式）
        </button>
      </div>
    </div>

    <!-- 已扫树 / 任务中 / 已完成 -->
    <template v-else>
      <div class="lr-work__bar">
        <span class="lr-work__url lr-mono lr-selectable" :title="task.wikiUrl">
          <AppIcon name="check-circle" :size="13" class="lr-work__urlok" />
          {{ task.wikiUrl }}
        </span>
        <button class="lr-btn lr-btn--ghost" :disabled="task.running" @click="task.clearAll()">
          换一个
        </button>
      </div>

      <div class="lr-work__body">
        <section class="lr-card lr-work__tree">
          <header class="lr-card__head">
            <span class="lr-card__title">节点树</span>
            <span class="lr-card__meta">
              {{ task.running ? "下载进行中，勾选已锁定" : "勾选需要的节点" }}
            </span>
          </header>
          <NodeTree
            :nodes="treeNodes"
            :selected="task.selectedTokens"
            :node-states="task.nodeStates"
            :disabled="task.running"
            @update:selected="task.selectedTokens = $event"
          />
        </section>

        <aside class="lr-card lr-work__side">
          <header class="lr-card__head">
            <span class="lr-card__title">{{ sideTitle }}</span>
          </header>

          <div class="lr-work__sidescroll">
            <!-- 已完成：结果卡 -->
            <ResultCard
              v-if="task.finished"
              :wiki-name="task.wikiUrl"
              :output-root="task.outputRoot"
              :items="task.items"
              :cancelled="task.cancelled"
              class="lr-work__result"
              @open-dir="task.reset()"
              @again="task.reset()"
            />

            <!-- 任务中：进度面板 -->
            <TaskPanel
              v-else-if="task.running"
              :phase="task.phase"
              :phase-label="task.phaseLabel"
              :done="task.done"
              :total="task.total"
              :success-count="task.successCount"
              :failed-count="task.failedCount"
              :current-doc="task.currentDoc"
              :estimated-remaining-seconds="task.estimatedRemainingSeconds"
              :cancelled="task.cancelled"
              :finished="false"
              @cancel="task.cancel()"
            />

            <!-- 已扫树：输出目录 + 摘要 + 开始 -->
            <div v-else class="lr-work__setup">
              <div class="lr-field">
                <span class="lr-field__label">输出目录</span>
                <DirPicker
                  v-model="settings.settings.output_dir"
                  :available-text="settings.availableText"
                />
              </div>

              <div class="lr-work__summary">
                <span class="lr-field__label">已选内容</span>
                <div class="lr-kv">
                  <span class="lr-kv__k">文档</span>
                  <span class="lr-kv__v">{{ task.selectedBreakdown.doc }}</span>
                </div>
                <div class="lr-kv">
                  <span class="lr-kv__k">表格</span>
                  <span class="lr-kv__v">{{ task.selectedBreakdown.sheet }}</span>
                </div>
                <div class="lr-kv">
                  <span class="lr-kv__k">多维表格</span>
                  <span class="lr-kv__v">{{ task.selectedBreakdown.bitable }}</span>
                </div>
                <div class="lr-kv">
                  <span class="lr-kv__k">附件</span>
                  <span class="lr-kv__v">{{ task.selectedBreakdown.file }}</span>
                </div>
                <div class="lr-kv lr-work__totalrow">
                  <span class="lr-kv__k">合计</span>
                  <span class="lr-kv__v">{{ task.selectedTokens.length }} 项</span>
                </div>
              </div>

              <div class="lr-work__opts">
                <label class="lr-work__switch">
                  <input v-model="settings.settings.download_images" type="checkbox" />
                  <span>下载文档中的图片</span>
                </label>
                <div class="lr-field">
                  <span class="lr-field__label">图片并发数 {{ settings.settings.concurrency }}</span>
                  <input
                    v-model.number="settings.settings.concurrency"
                    type="range"
                    min="1"
                    max="32"
                    class="lr-work__range"
                  />
                </div>
              </div>

              <button
                class="lr-btn lr-btn--primary lr-btn--lg lr-btn--block"
                :disabled="task.selectedTokens.length === 0"
                @click="onStart"
              >
                <AppIcon name="download" :size="14" />
                开始下载
              </button>
            </div>
          </div>
        </aside>
      </div>
    </template>
  </div>
</template>

<style scoped>
.lr-work {
  position: relative;
}

/* ---- 骨架演示条（接入 IPC 后删除） ---- */

/* ---- 空态 ---- */
.lr-work__empty {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.lr-work__emptycard {
  width: 460px;
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding: var(--lr-space-6);
  background: var(--lr-bg-surface);
  border: 0.5px solid var(--lr-border);
  border-radius: var(--lr-radius-xl);
}

.lr-work__emptylogo {
  width: 44px;
  height: 44px;
  border-radius: var(--lr-radius-lg);
  background: var(--lr-primary-soft);
  color: var(--lr-primary);
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: var(--lr-space-4);
}

.lr-work__emptytitle {
  font-size: var(--lr-fs-title);
  font-weight: var(--lr-fw-medium);
}

.lr-work__emptydesc {
  margin-top: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
}

.lr-work__emptyform {
  width: 100%;
  display: flex;
  gap: var(--lr-space-2);
  margin-top: var(--lr-space-5);
}

.lr-work__emptynote {
  margin-top: var(--lr-space-3);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

/* ---- 顶部链接条 ---- */
.lr-work__bar {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--lr-space-3);
  margin-bottom: var(--lr-space-3);
}

.lr-work__url {
  flex: 1;
  min-width: 0;
  display: inline-flex;
  align-items: center;
  gap: var(--lr-space-2);
  height: 32px;
  padding: 0 var(--lr-space-3);
  border: 0.5px solid var(--lr-border);
  border-radius: var(--lr-radius-md);
  background: var(--lr-bg-surface);
  color: var(--lr-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.lr-work__urlok {
  color: var(--lr-success);
  flex: none;
}

/* ---- 主体两栏 ---- */
.lr-work__body {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 320px;
  gap: var(--lr-space-4);
}

.lr-work__tree {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

.lr-work__side {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

.lr-work__sidescroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: var(--lr-space-4);
}

.lr-work__result {
  border: none;
  padding: 0;
}

.lr-work__setup {
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-5);
}

.lr-work__summary {
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-1);
  padding: var(--lr-space-3);
  border-radius: var(--lr-radius-md);
  background: var(--lr-bg-subtle);
}

.lr-work__totalrow {
  border-top: 0.5px solid var(--lr-border);
  margin-top: var(--lr-space-1);
  padding-top: var(--lr-space-2);
}

.lr-work__totalrow .lr-kv__v {
  font-weight: var(--lr-fw-medium);
}

.lr-work__opts {
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-3);
}

.lr-work__switch {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
  cursor: pointer;
}

.lr-work__switch input {
  accent-color: var(--lr-primary);
  cursor: pointer;
}

.lr-work__range {
  width: 100%;
  accent-color: var(--lr-primary);
  cursor: pointer;
}
</style>
