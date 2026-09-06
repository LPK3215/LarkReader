<script setup lang="ts">
// ============================================================================
// WorkspaceView —— 主工作台
//
// 三态单页流（不分步、不切页）：
//   empty   只等一个链接，居中卡片
//   tree    输入条收起为一行 + 左树勾选 + 右侧栏（输出目录 / 选中摘要 / 开始）
//   running 树不销毁，逐节点打勾打叉；右侧栏换成 TaskPanel
//   done    右侧栏换成 ResultCard，可「重新选择」回到 tree
// ============================================================================

import { computed, onMounted, ref } from "vue";
import { useTaskStore } from "../stores/task";
import { useSettingsStore } from "../stores/settings";
import type { ScanMode } from "../api/wiki";
import NodeTree from "../components/NodeTree.vue";
import TaskPanel from "../components/TaskPanel.vue";
import ResultCard from "../components/ResultCard.vue";
import DirPicker from "../components/DirPicker.vue";
import AppIcon from "../components/AppIcon.vue";
import { message } from "../composables/useMessage";

const task = useTaskStore();
const settings = useSettingsStore();

const inputUrl = ref("");
// 默认「展开整个知识库」：首次使用贴一个链接就能看到全貌，
// 需要精确控制时再手动切回「仅导出本节点及子树」。
const scanMode = ref<ScanMode>("full_space");

const treeNodes = computed(() => (task.tree ? [task.tree] : []));

// ---- 示例链接（首次使用自动展开一次，之后可随时点开） ----
const DEMO_SEEN_KEY = "larkreader_demo_hinted";
const showDemo = ref(false);
const demoLinks = [
  {
    label: "整库入口（推荐搭配「展开整个知识库」）",
    url: "https://qcny2iztd1p8.feishu.cn/wiki/LuQ7wEqgmiqITJkL49zcjyA0nif",
    mode: "full_space" as ScanMode,
  },
  {
    label: "目录文档示例（默认模式即可，含 3 个子文档）",
    url: "https://qcny2iztd1p8.feishu.cn/wiki/EAMOwgdxZiuJcGk7SggcXFGnnXf",
    mode: "auto" as ScanMode,
  },
];

onMounted(() => {
  try {
    if (!localStorage.getItem(DEMO_SEEN_KEY)) {
      showDemo.value = true;
      localStorage.setItem(DEMO_SEEN_KEY, "1");
    }
  } catch {
    /* localStorage 不可用（隐私模式等）时静默跳过 */
  }
});

/** 一键填入示例链接并按推荐模式扫描 */
async function useDemoLink(link: (typeof demoLinks)[number]) {
  inputUrl.value = link.url;
  scanMode.value = link.mode;
  showDemo.value = false;
  await onScan();
}

async function copyDemoLink(link: (typeof demoLinks)[number]) {
  try {
    await navigator.clipboard.writeText(link.url);
    message.success("示例链接已复制");
  } catch (err) {
    message.warning(`复制失败：${String(err)}`);
  }
}

/** Auto 扫描只得到单节点（无子文档）时的引导提示 */
const singleNodeScan = computed(() => {
  const t = task.tree;
  return (
    task.stage === "tree" &&
    !task.running &&
    task.scanMode === "auto" &&
    !!t &&
    t.children.length === 0
  );
});

/** 切换到 FullSpace 并用当前链接重扫 */
async function rescanFullSpace() {
  if (task.scanning) return;
  scanMode.value = "full_space";
  await task.scan(task.wikiUrl, "full_space");
}

const sideTitle = computed(() => {
  if (task.finished) return "导出结果";
  if (task.running) return "任务进度";
  return "导出设置";
});

async function onScan() {
  const url = inputUrl.value.trim();
  if (!url || task.scanning) return;
  await task.scan(url, scanMode.value);
}

/** 回车扫描：输入法组词回车不算 */
function onScanKey(event: KeyboardEvent) {
  if (event.isComposing) return;
  void onScan();
}

/** 用对话框选了新的输出目录：先预检，让可用空间与可写性提示立刻生效 */
function onDirPick(path: string) {
  void settings.refreshPreflight(path);
}

async function onStart() {
  if (task.starting) return; // store 内也有守卫，双保险
  // 工作台右侧的目录 / 下载图片 / 并发数直接改在 store 草稿上，启动前落盘，
  // 让后端任务读取与右侧展示一致的设置（后端按持久化配置执行）。
  try {
    await settings.save();
  } catch (err) {
    console.warn("[workspace] 启动前保存设置失败:", err);
    return;
  }
  await task.start();
}

/** 打开本次产物目录；未产出（如直接取消）时退回默认输出目录。 */
function openResultDir() {
  void settings.openDir(task.outputRoot || settings.settings.output_dir);
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
            :disabled="task.scanning"
            autofocus
            autocomplete="off"
            spellcheck="false"
            @keydown.enter="onScanKey"
          />
          <button
            class="lr-btn lr-btn--primary lr-btn--lg lr-work__scanbtn"
            :disabled="task.scanning || !inputUrl.trim()"
            @click="onScan"
          >
            <AppIcon
              v-if="task.scanning"
              name="spinner"
              :size="14"
              class="lr-icon-spin"
            />
            {{ task.scanning ? "扫描中…" : "扫描结构" }}
          </button>
        </div>

        <div class="lr-work__modeselect">
          <label class="lr-work__radio">
            <input
              v-model="scanMode"
              type="radio"
              value="auto"
              :disabled="task.scanning"
            />
            <span>仅导出本节点及子树</span>
          </label>
          <label class="lr-work__radio">
            <input
              v-model="scanMode"
              type="radio"
              value="full_space"
              :disabled="task.scanning"
            />
            <span>展开整个知识库（含兄弟节点）</span>
          </label>
        </div>

        <div class="lr-work__demo">
          <button class="lr-work__demotoggle" @click="showDemo = !showDemo">
            <AppIcon name="link" :size="12" />
            {{ showDemo ? "收起示例链接" : "没有链接？试试示例链接" }}
          </button>
          <div v-if="showDemo" class="lr-work__demolist">
            <p class="lr-work__demotip">
              这是开源的测试知识库，点「使用」自动填入并扫描，直观感受两种扫描模式的区别：
            </p>
            <div v-for="d in demoLinks" :key="d.url" class="lr-work__demorow">
              <div class="lr-work__demomain">
                <span class="lr-work__demolabel">{{ d.label }}</span>
                <code class="lr-work__demourl">{{ d.url }}</code>
              </div>
              <button
                class="lr-btn lr-btn--ghost"
                title="复制链接"
                @click="copyDemoLink(d)"
              >
                复制
              </button>
              <button
                class="lr-btn lr-btn--secondary"
                :disabled="task.scanning"
                @click="useDemoLink(d)"
              >
                使用
              </button>
            </div>
          </div>
        </div>

        <p class="lr-work__emptynote">
          <template v-if="task.scanning">正在读取知识库目录结构，请稍候…</template>
          <template v-else>扫描阶段只读取目录树，不下载正文、不写入磁盘</template>
        </p>
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
          <!-- Auto 模式只扫到单节点时的引导：一键切换 FullSpace 重扫 -->
          <div v-if="singleNodeScan" class="lr-work__hintbar">
            <AppIcon name="alert-circle" :size="13" />
            <span class="lr-work__hinttext">
              本次扫描只找到该节点自身——它在知识库目录树中没有子节点（旁边的节点是它的兄弟，不是它的子文档）。想要整个知识库？
            </span>
            <button
              class="lr-btn lr-btn--ghost lr-work__hintbtn"
              :disabled="task.scanning"
              @click="rescanFullSpace"
            >
              切换「展开整个知识库」重扫
            </button>
          </div>
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
              @open-dir="openResultDir"
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
                  @pick="onDirPick"
                />
              </div>

              <div class="lr-work__summary">
                <span class="lr-field__label">
                  将导出内容
                  <span v-if="task.counting" class="lr-work__summaryhint">计算中…</span>
                </span>
                <div class="lr-kv">
                  <span class="lr-kv__k">文档</span>
                  <span class="lr-kv__v">{{ task.exportableCount ? task.exportableCount.doc : "—" }}</span>
                </div>
                <div class="lr-kv">
                  <span class="lr-kv__k">表格</span>
                  <span class="lr-kv__v">{{ task.exportableCount ? task.exportableCount.sheet : "—" }}</span>
                </div>
                <div class="lr-kv">
                  <span class="lr-kv__k">多维表格</span>
                  <span class="lr-kv__v">{{ task.exportableCount ? task.exportableCount.bitable : "—" }}</span>
                </div>
                <div class="lr-kv">
                  <span class="lr-kv__k">附件</span>
                  <span class="lr-kv__v">{{ task.exportableCount ? task.exportableCount.file : "—" }}</span>
                </div>
                <div class="lr-kv lr-work__totalrow">
                  <span class="lr-kv__k">合计</span>
                  <span class="lr-kv__v">
                    {{
                      task.exportableCount
                        ? `${task.exportableCount.total} 篇/个`
                        : task.counting
                          ? "计算中…"
                          : "—"
                    }}
                  </span>
                </div>
                <div v-if="task.countError" class="lr-work__counterror">
                  {{ task.countError }}
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
                :disabled="task.selectedTokens.length === 0 || task.starting"
                @click="onStart"
              >
                <AppIcon
                  v-if="task.starting"
                  name="spinner"
                  :size="13"
                  class="lr-icon-spin"
                />
                <AppIcon v-else name="download" :size="14" />
                {{ task.starting ? "启动中…" : "开始下载" }}
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

/* ---- 示例链接 ---- */
.lr-work__demo {
  width: 100%;
  margin-top: var(--lr-space-2);
}

.lr-work__demotoggle {
  background: none;
  border: none;
  padding: 0;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: var(--lr-fs-secondary);
  color: var(--lr-primary);
  cursor: pointer;
}

.lr-work__demotoggle:hover {
  text-decoration: underline;
}

.lr-work__demolist {
  width: 100%;
  margin-top: var(--lr-space-2);
  padding: var(--lr-space-3);
  border: 0.5px dashed var(--lr-border);
  border-radius: var(--lr-radius-md);
  background: var(--lr-bg-subtle);
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-2);
  text-align: left;
}

.lr-work__demotip {
  margin: 0;
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-work__demorow {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
}

.lr-work__demomain {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.lr-work__demolabel {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text);
}

.lr-work__demourl {
  font-size: var(--lr-fs-mono);
  color: var(--lr-text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ---- 单节点扫描提示条 ---- */
.lr-work__hintbar {
  display: flex;
  align-items: flex-start;
  gap: var(--lr-space-2);
  margin: var(--lr-space-3) var(--lr-space-4) 0;
  padding: var(--lr-space-3);
  border-radius: var(--lr-radius-md);
  background: var(--lr-warning-soft, rgba(186, 117, 23, 0.08));
  border: 0.5px solid var(--lr-border);
  color: var(--lr-text-secondary);
  font-size: var(--lr-fs-secondary);
}

.lr-work__hinttext {
  flex: 1;
  min-width: 0;
  line-height: 1.6;
}

.lr-work__hintbtn {
  flex: none;
  color: var(--lr-primary);
}

.lr-work__modeselect {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-2);
  margin-top: var(--lr-space-3);
  padding: var(--lr-space-3);
  border-radius: var(--lr-radius-md);
  background: var(--lr-bg-subtle);
}

.lr-work__radio {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
  cursor: pointer;
}

.lr-work__radio input {
  accent-color: var(--lr-primary);
  cursor: pointer;
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

/* 计数计算中的角标提示 */
.lr-work__summaryhint {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
  font-weight: var(--lr-fw-regular);
}

/* 计数失败（如勾选时机异常）的红色提示 */
.lr-work__counterror {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-danger);
  margin-top: var(--lr-space-1);
  word-break: break-all;
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