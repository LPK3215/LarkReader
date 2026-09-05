<script setup lang="ts">
// ============================================================================
// AppConfigGuide —— 飞书应用配置引导面板
//
// 背景：lark-cli 需要绑定一个「飞书自建应用」才能访问文档，本工具不带固定应用，
// 需用户在自己的开放平台账号下创建并回填给 lark-cli，一次配置永久生效。
//
// 两条路径：
//   1. 【推荐】一键自动创建：调 start_app_init（后端后台跑 `config init --new`，
//      逐行流式输出），轮询 get_app_init_status 抓到 lark-cli 打印的创建向导
//      URL 后自动打开浏览器；用户在浏览器完成创建，后端命令随之结束，自动回抛
//      recheck 让父组件重跑环境体检，闭环全程无需手动输入。
//   2. 【兜底】手动方式：给出等价的完整命令 + 操作步骤，用户在自己终端跑。
//
// 父组件通过 v-if 控制显隐；本组件回抛 close / recheck（onboarding / terminal
// 各自的体检）。
// ============================================================================

import { onBeforeUnmount, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import AppIcon from "./AppIcon.vue";
import { message } from "../composables/useMessage";
import { getAppInitStatus, startAppInit } from "../api/env";

defineProps<{ busy?: boolean }>();
const emit = defineEmits<{
  (e: "close"): void;
  (e: "recheck"): void;
}>();

/** 终端手动创建飞书应用的兜底命令（与应用内 start_app_init 走同一 lark-cli 向导） */
const INIT_CMD = "lark-cli config init --new --brand feishu --lang zh";
const OPEN_PLATFORM_URL = "https://open.feishu.cn/app";

const copied = ref(false);
const urlCopied = ref(false);
let copyTimer: number | undefined;

/** 自动创建流程的本地状态 */
const working = ref(false);
const linkFound = ref("");
const openedOnce = ref(false);
const autoDone = ref(false);
const liveText = ref("");
const autoFail = ref("");
let pollTimer: number | undefined;

async function startAuto() {
  if (working.value) return;
  working.value = true;
  autoDone.value = false;
  autoFail.value = "";
  linkFound.value = "";
  openedOnce.value = false;
  liveText.value = "正在启动飞书创建向导…";
  try {
    const status = await startAppInit();
    consumeStatus(status);
    if (working.value) {
      pollTimer = window.setInterval(pollStatus, 700);
    }
  } catch (err) {
    working.value = false;
    autoFail.value = errText(err);
    message.warning("自动创建未能启动，请改用下方手动方式");
  }
}

async function pollStatus() {
  if (!working.value) return;
  try {
    consumeStatus(await getAppInitStatus());
  } catch (err) {
    stopPoll();
    working.value = false;
    autoFail.value = errText(err);
  }
}

function consumeStatus(status: {
  running: boolean;
  stage: string;
  url: string | null;
  message: string | null;
  error: string | null;
}) {
  if (status.url && !linkFound.value) {
    linkFound.value = status.url;
    void openLink();
  }
  if (!status.running) {
    stopPoll();
    working.value = false;
    if (status.error) {
      autoFail.value = status.error;
      liveText.value = "";
    } else {
      autoDone.value = true;
      liveText.value = "创建成功，正在自动重新检测…";
      emit("recheck");
    }
    return;
  }
  liveText.value = status.stage || status.message || "正在等待向导完成…";
}

async function openLink() {
  if (!linkFound.value || openedOnce.value) return;
  openedOnce.value = true;
  liveText.value = "已生成创建链接，正在自动打开浏览器…";
  try {
    await openUrl(linkFound.value);
  } catch {
    message.warning("自动打开浏览器失败，请点下方「重新打开链接」");
  }
}

async function openLinkManually() {
  if (!linkFound.value) return;
  try {
    await openUrl(linkFound.value);
  } catch {
    message.warning("无法自动打开浏览器，请在浏览器中访问上方链接");
  }
}

function stopPoll() {
  if (pollTimer !== undefined) {
    window.clearInterval(pollTimer);
    pollTimer = undefined;
  }
}

function errText(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err);
  // 去掉 Tauri invoke 拼接的调试壳，取真正的人类可读消息
  const m = raw.match(/"message":"([^"]*)"/);
  return m ? m[1] : raw;
}

async function copyText(text: string, label: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // 剪贴板 API 被拒时退回 execCommand 老方案
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.setAttribute("readonly", "");
    ta.style.position = "fixed";
    ta.style.left = "-9999px";
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    ta.remove();
    if (!ok) return;
  }
  if (label === "cmd") {
    copied.value = true;
    message.success("命令已放入剪贴板，去终端粘贴运行");
  } else {
    urlCopied.value = true;
    message.success("链接已放入剪贴板");
  }
  window.clearTimeout(copyTimer);
  copyTimer = window.setTimeout(() => {
    copied.value = false;
    urlCopied.value = false;
  }, 2400);
}

function copyCommand() {
  void copyText(INIT_CMD, "cmd");
}

function copyLink() {
  if (linkFound.value) void copyText(linkFound.value, "url");
}

async function openAppCenter() {
  try {
    await openUrl(OPEN_PLATFORM_URL);
  } catch {
    message.warning(`无法打开浏览器，请手动访问 ${OPEN_PLATFORM_URL}`);
  }
}

function onClose() {
  stopPoll();
  window.clearTimeout(copyTimer);
  emit("close");
}

onBeforeUnmount(stopPoll);
</script>

<template>
  <section class="lr-appcfg">
    <header class="lr-appcfg__head">
      <span class="lr-appcfg__title">
        <AppIcon name="alert-circle" :size="14" />
        配置飞书自建应用
      </span>
      <button class="lr-appcfg__close" aria-label="收起引导" @click="onClose">
        <AppIcon name="close" :size="13" />
      </button>
    </header>

    <p class="lr-appcfg__why">
      本工具通过 lark-cli 访问飞书文档，需要先绑定一个「飞书自建应用」。以下操作
      只需做一次，之后一直有效。
    </p>

    <!-- 自动创建路径 -->
    <div class="lr-appcfg__auto">
      <div v-if="!working && !autoDone" class="lr-appcfg__autohead">
        <button
          class="lr-btn lr-btn--primary lr-appcfg__autoBtn"
          :disabled="busy"
          @click="startAuto"
        >
          <AppIcon name="plus" :size="13" />
          一键自动创建并打开浏览器
        </button>
        <span class="lr-appcfg__autohint"
          >无需手动运行命令，向导会自动弹到浏览器，完成即回到本页检测。</span
        >
      </div>

      <!-- 运行中 / 已完成 / 失败 共用状态区 -->
      <div v-if="working || autoDone || autoFail" class="lr-appcfg__statusbox">
        <div class="lr-appcfg__statusline" :class="autoDone ? 'is-done' : autoFail ? 'is-fail' : ''">
          <AppIcon v-if="working" class="lr-icon-spin" name="spinner" :size="13" />
          <AppIcon v-else-if="autoDone" name="check-circle" :size="13" />
          <AppIcon v-else name="close-circle" :size="13" />
          <span>{{ autoFail ? `创建未完成：${autoFail}` : liveText }}</span>
        </div>

        <div v-if="linkFound && working" class="lr-appcfg__linkrow">
          <span class="lr-appcfg__linklabel">向导链接：</span>
          <code class="lr-appcfg__cmdcode lr-selectable">{{ linkFound }}</code>
          <button class="lr-btn lr-btn--secondary lr-appcfg__mini" @click="openLinkManually">
            重新打开链接
          </button>
          <button
            class="lr-btn lr-btn--secondary lr-appcfg__mini"
            @click="copyLink"
          >
            {{ urlCopied ? "已放入" : "链接放入剪贴板" }}
          </button>
        </div>

        <div v-if="working && linkFound" class="lr-appcfg__stepnote">
          浏览器已打开时，跟随页面完成创建即可；完成后本面板会自动继续并重新检测，
          无需再手动操作。
        </div>
        <div v-if="autoDone" class="lr-appcfg__stepnote">
          环境检测刷新中，几秒后状态会变绿。
        </div>
        <div v-if="autoFail" class="lr-appcfg__stepnote">
          可以点上方按钮重试；若反复失败，请用下方「手动方式」在终端运行。
        </div>
      </div>
    </div>

    <!-- 手动方式（自动运行/成功后收起，失败时保留作兜底） -->
    <template v-if="!working && !autoDone">
      <div class="lr-appcfg__divider">
        <span>或手动方式（兜底）</span>
      </div>

      <ol class="lr-appcfg__steps">
        <li>打开电脑终端（PowerShell、终端.app、VS Code 终端都可以）。</li>
        <li>
          粘贴并运行下面这条命令：
          <div class="lr-appcfg__cmd">
            <code class="lr-appcfg__cmdcode lr-selectable">{{ INIT_CMD }}</code>
            <button class="lr-btn lr-btn--secondary lr-appcfg__copy" @click="copyCommand">
              <AppIcon name="doc" :size="12" />
              {{ copied ? "已放入" : "命令放入剪贴板" }}
            </button>
          </div>
        </li>
        <li>
          命令会打开浏览器并进入飞书开放平台创建向导，跟随提示完成自建应用的创建
          （会复用你的飞书账号授权，通常几十秒）。此期间请勿关闭终端。
        </li>
        <li>
          看到创建完成后，回到本窗口点下方「重新检测」，状态变绿即可继续。
        </li>
      </ol>

      <p class="lr-appcfg__note">
        提示：若提示找不到 <code class="lr-appcfg__inline">lark-cli</code>，先重开
        终端让 PATH 生效；想手动在网页创建也可以，但建好后仍需跑上面命令把
        App ID / Secret 写回 lark-cli，所以直接跑命令最省事。
      </p>
    </template>

    <footer class="lr-appcfg__actions">
      <button class="lr-btn lr-btn--ghost lr-appcfg__action" @click="openAppCenter">
        <AppIcon name="external" :size="13" />
        打开开放平台
      </button>
      <span class="lr-appcfg__spacer" />
      <button
        class="lr-btn lr-btn--primary lr-appcfg__action"
        :disabled="busy || working"
        @click="emit('recheck')"
      >
        {{ busy ? "检测中…" : "我已完成，重新检测" }}
      </button>
    </footer>
  </section>
</template>

<style scoped>
.lr-appcfg {
  margin-top: var(--lr-space-3);
  padding: var(--lr-space-4);
  border: 0.5px dashed var(--lr-warning-border);
  border-radius: var(--lr-radius-lg);
  background: var(--lr-warning-soft);
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-3);
}

.lr-appcfg__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--lr-space-3);
}

.lr-appcfg__title {
  display: inline-flex;
  align-items: center;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-section);
  font-weight: var(--lr-fw-medium);
  color: var(--lr-warning);
}

.lr-appcfg__close {
  flex: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: none;
  border-radius: var(--lr-radius-sm);
  background: transparent;
  color: var(--lr-text-tertiary);
}

.lr-appcfg__close:hover {
  background: var(--lr-bg-hover);
  color: var(--lr-text);
}

.lr-appcfg__why {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
  line-height: var(--lr-lh-body);
}

/* ---- 自动创建 ---- */
.lr-appcfg__auto {
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-2);
}

.lr-appcfg__autohead {
  display: flex;
  align-items: center;
  gap: var(--lr-space-3);
}

.lr-appcfg__autoBtn {
  height: 30px;
  padding: 0 var(--lr-space-4);
  font-size: var(--lr-fs-secondary);
}

.lr-appcfg__autohint {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
}

.lr-appcfg__statusbox {
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-2);
  padding: var(--lr-space-2) var(--lr-space-3);
  border-radius: var(--lr-radius-md);
  background: var(--lr-bg-surface);
  border: 0.5px solid var(--lr-border);
}

.lr-appcfg__statusline {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
  word-break: break-all;
}

.lr-appcfg__statusline.is-done {
  color: var(--lr-success);
}

.lr-appcfg__statusline.is-fail {
  color: var(--lr-danger);
}

.lr-appcfg__linkrow {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  flex-wrap: wrap;
}

.lr-appcfg__linklabel {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
  flex: none;
}

.lr-appcfg__mini {
  flex: none;
  height: 24px;
  padding: 0 var(--lr-space-3);
  font-size: var(--lr-fs-secondary);
}

.lr-appcfg__stepnote {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
  line-height: var(--lr-lh-body);
}

/* ---- 手动方式分隔线 ---- */
.lr-appcfg__divider {
  display: flex;
  align-items: center;
  gap: var(--lr-space-3);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-appcfg__divider::before,
.lr-appcfg__divider::after {
  content: "";
  flex: 1;
  height: 0.5px;
  background: var(--lr-border);
}

/* ---- 手动步骤（沿用原样式） ---- */
.lr-appcfg__steps {
  margin: 0;
  padding-left: var(--lr-space-4);
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-3);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
  line-height: var(--lr-lh-body);
}

.lr-appcfg__cmd {
  display: flex;
  align-items: center;
  gap: var(--lr-space-3);
  margin-top: var(--lr-space-2);
  padding: var(--lr-space-2) var(--lr-space-3);
  border-radius: var(--lr-radius-md);
  background: var(--lr-bg-subtle);
}

.lr-appcfg__cmdcode {
  flex: 1;
  min-width: 0;
  color: var(--lr-text);
  word-break: break-all;
}

.lr-appcfg__copy {
  flex: none;
  height: 24px;
  padding: 0 var(--lr-space-3);
  font-size: var(--lr-fs-secondary);
}

.lr-appcfg__note {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
  line-height: var(--lr-lh-body);
}

.lr-appcfg__inline {
  color: var(--lr-text-secondary);
}

.lr-appcfg__actions {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
}

.lr-appcfg__spacer {
  flex: 1;
}

.lr-appcfg__action {
  height: 28px;
  padding: 0 var(--lr-space-3);
  font-size: var(--lr-fs-secondary);
}
</style>
