<script setup lang="ts">
// ============================================================================
// AppConfigGuide —— 飞书应用配置引导面板（轻量止血版）
//
// 背景：lark-cli 需要绑定一个「飞书自建应用」才能访问文档，本工具不带固定
// 应用，需用户在自己的开放平台账号下创建并回填给 lark-cli，一次配置永久生效。
//
// 现状：后端 init_app（config init --new）是同步命令，UI 直调会占死 IPC 600s，
// 因此本引导不调用 init_app，而是给出等价的手动命令 + 操作步骤：
//   1. 复制命令（与后端 init_app 同一命令）
//   2. 用户在终端粘贴运行，浏览器打开飞书开放平台创建向导
//   3. 创建完成后回到原页面点「重新检测」完成闭环
//
// 父组件通过 v-if 控制显隐；本组件只负责文案、复制命令、打开开放平台，
// 以及把「重新检测」动作回抛给父组件（onboarding 或 terminal 各自的体检）。
// ============================================================================

import { ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import AppIcon from "./AppIcon.vue";
import { message } from "../composables/useMessage";

defineProps<{ busy?: boolean }>();
const emit = defineEmits<{
  (e: "close"): void;
  (e: "recheck"): void;
}>();

/** 与后端 init_app 同一命令：阻塞式创建向导，完成即把 App ID/Secret 写回 lark-cli。 */
const INIT_CMD = "lark-cli config init --new --brand feishu --lang zh";
const OPEN_PLATFORM_URL = "https://open.feishu.cn/app";

const copied = ref(false);
let copyTimer: number | undefined;

async function copyCommand() {
  try {
    await navigator.clipboard.writeText(INIT_CMD);
  } catch {
    // 剪贴板 API 被拒时退回 execCommand 老方案
    const ta = document.createElement("textarea");
    ta.value = INIT_CMD;
    ta.setAttribute("readonly", "");
    ta.style.position = "fixed";
    ta.style.left = "-9999px";
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    ta.remove();
    if (!ok) return;
  }
  copied.value = true;
  message.success("命令已复制，去终端粘贴运行");
  window.clearTimeout(copyTimer);
  copyTimer = window.setTimeout(() => {
    copied.value = false;
  }, 2400);
}

async function openAppCenter() {
  try {
    await openUrl(OPEN_PLATFORM_URL);
  } catch {
    message.warning(`无法打开浏览器，请手动访问 ${OPEN_PLATFORM_URL}`);
  }
}

function onClose() {
  window.clearTimeout(copyTimer);
  emit("close");
}
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

    <ol class="lr-appcfg__steps">
      <li>打开电脑终端（PowerShell、终端.app、VS Code 终端都可以）。</li>
      <li>
        粘贴并运行下面这条命令：
        <div class="lr-appcfg__cmd">
          <code class="lr-appcfg__cmdcode lr-selectable">{{ INIT_CMD }}</code>
          <button
            class="lr-btn lr-btn--secondary lr-appcfg__copy"
            @click="copyCommand"
          >
            <AppIcon name="doc" :size="12" />
            {{ copied ? "已复制" : "复制命令" }}
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

    <footer class="lr-appcfg__actions">
      <button class="lr-btn lr-btn--ghost lr-appcfg__action" @click="openAppCenter">
        <AppIcon name="external" :size="13" />
        打开开放平台
      </button>
      <span class="lr-appcfg__spacer" />
      <button
        class="lr-btn lr-btn--primary lr-appcfg__action"
        :disabled="busy"
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
