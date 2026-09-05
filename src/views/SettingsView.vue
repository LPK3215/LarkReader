<script setup lang="ts">
// ============================================================================
// SettingsView —— 设置页
//
// 表单：默认输出目录（DirPicker + preflight）、图片并发数（1–32）、是否下载图片
// 读写：api/settings.ts + stores/settings.ts；展示 SettingsStatus.warning
//       （配置损坏时后端会备份原文件、恢复默认设置，并通过该字段给出警告）
//
// 保存按钮：把当前 settings 草稿写盘；恢复默认：把草稿重置为 DEFAULT，再写盘。
// ============================================================================

import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useSettingsStore, DEFAULT_SETTINGS } from "../stores/settings";
import DirPicker from "../components/DirPicker.vue";
import AppIcon from "../components/AppIcon.vue";
import { message } from "../composables/useMessage";
import {
  checkForUpdate,
  downloadAndInstallUpdate,
  getCurrentVersion,
  type UpdateProgress,
} from "../api/updater";

const settings = useSettingsStore();

const saving = ref(false);
/** 保存成功的瞬时内联反馈（替代无处安放的 toast） */
const flashText = ref("");
let flashTimer: number | undefined;

function flashSaved(text: string) {
  flashText.value = text;
  if (flashTimer) window.clearTimeout(flashTimer);
  flashTimer = window.setTimeout(() => {
    flashText.value = "";
  }, 1600);
}

onBeforeUnmount(() => {
  if (flashTimer) window.clearTimeout(flashTimer);
});

async function onSave() {
  if (saving.value) return;
  saving.value = true;
  try {
    await settings.save();
    flashSaved("保存成功");
  } catch (err) {
    // 写盘失败：settings.warning 已同步，顶部告警横幅会展示具体原因
    console.warn("[settings] 保存失败:", err);
  } finally {
    saving.value = false;
  }
}

async function onReset() {
  if (saving.value) return;
  saving.value = true;
  try {
    await settings.save({ ...DEFAULT_SETTINGS });
    flashSaved("保存成功");
  } catch (err) {
    console.warn("[settings] 恢复默认失败:", err);
  } finally {
    saving.value = false;
  }
}

/** 用对话框选了新目录：先做可写性预检，让「可用空间/错误」立刻刷新 */
function onDirPick(path: string) {
  void settings.refreshPreflight(path);
}

// ---- 软件更新 ----
const checking = ref(false);
const installing = ref(false);
const checkError = ref("");
const currentVersion = ref("");
const nextVersion = ref("");
type UpdateUiState = "idle" | "fresh" | "hasUpdate";
const updateState = ref<UpdateUiState>("idle");
const installProgress = ref<UpdateProgress | null>(null);

const installPercent = computed(() => {
  const p = installProgress.value;
  if (!p || !p.total) return null;
  return p.total > 0 ? Math.min(100, Math.round((p.downloaded / p.total) * 100)) : 0;
});

async function onCheckUpdate() {
  if (checking.value || installing.value) return;
  checking.value = true;
  updateState.value = "idle";
  checkError.value = "";
  try {
    const result = await checkForUpdate();
    currentVersion.value = result.currentVersion;
    if (result.kind === "available") {
      nextVersion.value = result.nextVersion;
      updateState.value = "hasUpdate";
    } else if (result.kind === "error") {
      checkError.value = result.message;
    } else {
      updateState.value = "fresh";
    }
  } finally {
    checking.value = false;
  }
}

async function onInstallUpdate() {
  if (installing.value) return;
  installing.value = true;
  installProgress.value = { downloaded: 0, finished: false };
  checkError.value = "";
  try {
    await downloadAndInstallUpdate((progress) => {
      installProgress.value = progress;
    });
    // mac / Linux：安装完成并 relaunch，正常不会走到这里之外；若到达则说明没触发重启
    message.info("新版本已安装，应用即将重启");
  } catch (err) {
    checkError.value = err instanceof Error ? err.message : String(err);
    message.warning("更新失败，请稍后重试");
  } finally {
    installing.value = false;
  }
}

onMounted(async () => {
  await settings.load();
  const version = await getCurrentVersion();
  if (version) currentVersion.value = version;
});
</script>

<template>
  <div class="lr-page">
    <header class="lr-page__head">
      <h1 class="lr-page__title">设置</h1>
      <p class="lr-page__desc">导出行为与输出位置</p>
    </header>

    <div class="lr-page__body lr-settings">
      <p v-if="settings.warning" class="lr-settings__warning">
        <AppIcon name="alert-circle" :size="14" />
        {{ settings.warning }}
      </p>

      <section class="lr-card">
        <header class="lr-card__head">
          <span class="lr-card__title">输出位置</span>
        </header>
        <div class="lr-card__body">
          <div class="lr-field">
            <span class="lr-field__label">默认输出目录</span>
            <DirPicker
              v-model="settings.settings.output_dir"
              :available-text="settings.availableText"
              @pick="onDirPick"
            />
            <span class="lr-field__hint">
              每次导出会在该目录下新建以知识库名命名的子目录，同名不会覆盖
            </span>
          </div>
        </div>
      </section>

      <section class="lr-card">
        <header class="lr-card__head">
          <span class="lr-card__title">导出行为</span>
        </header>
        <div class="lr-card__body lr-settings__group">
          <label class="lr-settings__switch">
            <input v-model="settings.settings.download_images" type="checkbox" />
            <span class="lr-settings__switchtext">
              下载文档中的图片
              <em class="lr-settings__hint">
                关闭后只保留 Markdown 文本，图片仍保留原始链接
              </em>
            </span>
          </label>

          <div class="lr-field">
            <span class="lr-field__label">
              图片并发下载数
              <b class="lr-settings__value">{{ settings.settings.concurrency }}</b>
            </span>
            <input
              v-model.number="settings.settings.concurrency"
              type="range"
              min="1"
              max="32"
              class="lr-settings__range"
            />
            <span class="lr-field__hint">范围 1–32，网络较差时调低更稳</span>
          </div>
        </div>
      </section>

      <div class="lr-settings__footer">
        <button class="lr-btn lr-btn--secondary" :disabled="saving" @click="onReset">
          恢复默认
        </button>
        <button class="lr-btn lr-btn--primary" :disabled="saving" @click="onSave">
          {{ saving ? "保存中…" : flashText || "保存" }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.lr-settings {
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-4);
  padding-right: var(--lr-space-1);
}

.lr-settings__warning {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  padding: var(--lr-space-3) var(--lr-space-4);
  border-radius: var(--lr-radius-md);
  background: var(--lr-warning-soft);
  border: 0.5px solid var(--lr-warning-border);
  color: var(--lr-warning);
  font-size: var(--lr-fs-secondary);
}

.lr-settings__group {
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-5);
}

.lr-settings__switch {
  display: flex;
  align-items: flex-start;
  gap: var(--lr-space-3);
  cursor: pointer;
}

.lr-settings__switch input {
  margin-top: 2px;
  accent-color: var(--lr-primary);
  cursor: pointer;
}

.lr-settings__switchtext {
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-1);
  font-size: var(--lr-fs-body);
}

.lr-settings__hint {
  font-style: normal;
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-settings__value {
  color: var(--lr-primary);
  font-weight: var(--lr-fw-medium);
}

.lr-settings__range {
  width: 100%;
  accent-color: var(--lr-primary);
  cursor: pointer;
}

.lr-settings__footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--lr-space-2);
}
</style>