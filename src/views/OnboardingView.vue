<script setup lang="ts">
// ============================================================================
// OnboardingView —— 环境体检 / 登录 / 默认输出目录引导
//
// 全屏无壳（路由 meta.bare），完成即跳转 /workspace，不再出现。
// 流程：check_env ->（缺 lark-cli 则 setup_lark_cli）-> 登录 -> 选默认输出目录
//
// 三个硬前置必须在进入工作台前解决：
//   1. Node.js + lark-cli 可用     2. 飞书已登录     3. 有可写的默认输出目录
// 登录走两步非阻塞（start_login 拿设备码 -> 浏览器授权 -> 轮询 complete_login）。
// 浏览器 dev 环境（isTauri() === false）保留 demo 数据，便于视觉验收。
// ============================================================================

import { onBeforeUnmount, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useSettingsStore } from "../stores/settings";
import { useOnboardingStore, type CheckState } from "../stores/onboarding";
import { openUrl } from "@tauri-apps/plugin-opener";
import DirPicker from "../components/DirPicker.vue";
import AppIcon from "../components/AppIcon.vue";

const router = useRouter();
const settings = useSettingsStore();
const onboarding = useOnboardingStore();

const STEPS = ["环境体检", "登录飞书", "输出目录", "完成"];

const ICON: Record<CheckState, string> = {
  pending: "info-circle",
  ok: "check-circle",
  warn: "alert-circle",
  error: "close-circle",
};

const ICON_CLASS: Record<CheckState, string> = {
  pending: "is-pending",
  ok: "is-ok",
  warn: "is-warn",
  error: "is-error",
};

const recheckBusy = ref(false);
async function recheck() {
  recheckBusy.value = true;
  try {
    await onboarding.runCheck();
  } finally {
    recheckBusy.value = false;
  }
}

async function fixCli() {
  await onboarding.installCli();
}

async function onBeginLogin() {
  await onboarding.beginLogin();
}

async function openVerificationUrl() {
  if (!onboarding.verificationUrl) return;
  try {
    await openUrl(onboarding.verificationUrl);
  } catch {
    /* 权限失败时设备码仍可见 */
  }
}

function cancelLogin() {
  onboarding.cancelLogin();
}

function nextStep() {
  onboarding.step = onboarding.step + 1;
}
function prevStep() {
  onboarding.step = Math.max(0, onboarding.step - 1);
}

function finish() {
  router.push("/workspace");
}

onMounted(async () => {
  await Promise.all([settings.load(), onboarding.runCheck()]);
});

onBeforeUnmount(() => {
  onboarding.cancelLogin();
});
</script>

<template>
  <div class="lr-onboard">
    <div class="lr-onboard__box">
      <header class="lr-onboard__brand">
        <span class="lr-onboard__logo"><AppIcon name="doc" :size="16" /></span>
        <span class="lr-onboard__name">LarkReader</span>
        <span class="lr-onboard__tag">飞书文档本地导出</span>
      </header>

      <!-- 步骤条 -->
      <ol class="lr-onboard__steps">
        <li
          v-for="(s, i) in STEPS"
          :key="s"
          class="lr-onboard__step"
          :class="{ 'is-done': i < onboarding.step, 'is-current': i === onboarding.step }"
        >
          <span class="lr-onboard__dot">{{ i < onboarding.step ? "✓" : i + 1 }}</span>
          <span>{{ s }}</span>
        </li>
      </ol>

      <div class="lr-onboard__body">
        <!-- 步骤 1：环境体检 -->
        <template v-if="onboarding.step === 0">
          <h2 class="lr-onboard__title">检查运行环境</h2>
          <p class="lr-onboard__desc">
            导出依赖 Node.js 与固定版本的 lark-cli，缺项会自动安装
          </p>

          <ul v-if="onboarding.checks.length > 0" class="lr-onboard__checks">
            <li v-for="item in onboarding.checks" :key="item.key" class="lr-onboard__check">
              <span class="lr-onboard__state" :class="ICON_CLASS[item.state]">
                <AppIcon :name="ICON[item.state]" :size="14" />
              </span>
              <span class="lr-onboard__checkmain">
                <span class="lr-onboard__checklabel">{{ item.label }}</span>
                <span class="lr-onboard__checkdetail">{{ item.detail }}</span>
              </span>
              <button
                v-if="item.action && item.key === 'cli'"
                class="lr-btn lr-btn--secondary lr-onboard__fix"
                :disabled="onboarding.checking"
                @click="fixCli"
              >
                {{ item.action }}
              </button>
            </li>
          </ul>
          <p v-else class="lr-onboard__desc">尚未检测</p>
        </template>

        <!-- 步骤 2：登录 -->
        <template v-else-if="onboarding.step === 1">
          <h2 class="lr-onboard__title">登录飞书账号</h2>
          <p class="lr-onboard__desc">
            用你自己的飞书账号授权只读权限，凭据由 lark-cli 保管，本应用不保存密码
          </p>

          <!-- 已登录 -->
          <div v-if="onboarding.loginState === 'done'" class="lr-onboard__done">
            <AppIcon name="check-circle" :size="16" class="lr-onboard__doneicon" />
            <span>已登录为 <b>{{ onboarding.userName }}</b></span>
            <button class="lr-btn lr-btn--ghost" @click="onBeginLogin">切换账号</button>
          </div>

          <!-- 等待授权 -->
          <div v-else-if="onboarding.loginState === 'awaiting'" class="lr-onboard__device">
            <p class="lr-onboard__devlabel">在浏览器中打开下面链接，并输入设备码</p>
            <code class="lr-onboard__code lr-selectable">{{ onboarding.deviceCode }}</code>
            <button class="lr-btn lr-btn--primary lr-btn--lg" @click="openVerificationUrl">
              <AppIcon name="external" :size="14" />
              打开浏览器授权
            </button>
            <p class="lr-onboard__wait">
              <AppIcon name="spinner" :size="12" class="lr-onboard__spin" />
              等待授权完成…
            </p>
            <code class="lr-onboard__url lr-selectable">{{ onboarding.verificationUrl }}</code>
            <button class="lr-btn lr-btn--ghost" @click="cancelLogin">取消</button>
          </div>

          <!-- 失败 -->
          <div v-else-if="onboarding.loginState === 'failed'" class="lr-onboard__device lr-onboard__device--failed">
            <p class="lr-onboard__devlabel">登录失败</p>
            <p class="lr-onboard__desc">{{ onboarding.loginError }}</p>
            <button class="lr-btn lr-btn--primary" @click="onBeginLogin">重试</button>
          </div>

          <!-- 初始 -->
          <div v-else class="lr-onboard__device">
            <p class="lr-onboard__devlabel">使用设备码登录，本应用不接触你的账号密码</p>
            <button class="lr-btn lr-btn--primary lr-btn--lg" @click="onBeginLogin">
              <AppIcon name="external" :size="14" />
              开始登录
            </button>
          </div>
        </template>

        <!-- 步骤 3：输出目录 -->
        <template v-else-if="onboarding.step === 2">
          <h2 class="lr-onboard__title">选择导出位置</h2>
          <p class="lr-onboard__desc">
            之后每次导出会在该目录下新建以知识库名命名的子目录，随时可在设置里改
          </p>

          <div class="lr-onboard__dir">
            <DirPicker
              v-model="settings.settings.output_dir"
              :available-text="settings.availableText"
            />
          </div>
        </template>

        <!-- 步骤 4：完成 -->
        <template v-else>
          <div class="lr-onboard__finish">
            <span class="lr-onboard__finishicon"><AppIcon name="check" :size="26" /></span>
            <h2 class="lr-onboard__title">一切就绪</h2>
            <p class="lr-onboard__desc">
              环境正常 · 已登录 {{ onboarding.userName ?? "—" }} · 导出到
              <code class="lr-onboard__inlinepath lr-selectable">
                {{ settings.settings.output_dir }}
              </code>
            </p>
            <button class="lr-btn lr-btn--primary lr-btn--lg" @click="finish">
              进入工作台
            </button>
          </div>
        </template>
      </div>

      <footer class="lr-onboard__foot">
        <button
          v-if="onboarding.step > 0"
          class="lr-btn lr-btn--ghost"
          @click="prevStep"
        >
          上一步
        </button>
        <span class="lr-onboard__spacer" />
        <button
          v-if="onboarding.step === 0"
          class="lr-btn lr-btn--secondary"
          :disabled="recheckBusy"
          @click="recheck"
        >
          {{ recheckBusy ? "检测中…" : "重新检测" }}
        </button>
        <button
          v-if="onboarding.step < 3"
          class="lr-btn lr-btn--primary"
          :disabled="
            (onboarding.step === 0 && !onboarding.envReady) ||
            (onboarding.step === 1 && onboarding.loginState !== 'done')
          "
          @click="nextStep"
        >
          下一步
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.lr-onboard {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--lr-bg-app);
}

.lr-onboard__box {
  width: 560px;
  background: var(--lr-bg-surface);
  border: 0.5px solid var(--lr-border);
  border-radius: var(--lr-radius-xl);
  padding: var(--lr-space-6);
  display: flex;
  flex-direction: column;
}

.lr-onboard__brand {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
}

.lr-onboard__logo {
  width: 24px;
  height: 24px;
  border-radius: var(--lr-radius-md);
  background: var(--lr-primary);
  color: var(--lr-on-primary);
  display: flex;
  align-items: center;
  justify-content: center;
}

.lr-onboard__name {
  font-size: var(--lr-fs-section);
  font-weight: var(--lr-fw-medium);
}

.lr-onboard__tag {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

/* ---- 步骤条 ---- */
.lr-onboard__steps {
  list-style: none;
  margin: var(--lr-space-5) 0;
  padding: 0;
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
}

.lr-onboard__step {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-onboard__step + .lr-onboard__step::before {
  content: "";
  width: 24px;
  height: 1px;
  background: var(--lr-border);
  margin-right: var(--lr-space-2);
}

.lr-onboard__dot {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--lr-fs-mono);
  background: var(--lr-bg-subtle);
  color: var(--lr-text-tertiary);
}

.lr-onboard__step.is-current {
  color: var(--lr-text);
  font-weight: var(--lr-fw-medium);
}

.lr-onboard__step.is-current .lr-onboard__dot {
  background: var(--lr-primary);
  color: var(--lr-on-primary);
}

.lr-onboard__step.is-done {
  color: var(--lr-text-secondary);
}

.lr-onboard__step.is-done .lr-onboard__dot {
  background: var(--lr-success-soft);
  color: var(--lr-success);
}

/* ---- 内容区 ---- */
.lr-onboard__body {
  min-height: 220px;
}

.lr-onboard__title {
  font-size: var(--lr-fs-title);
  font-weight: var(--lr-fw-medium);
}

.lr-onboard__desc {
  margin-top: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
  line-height: var(--lr-lh-body);
}

.lr-onboard__checks {
  list-style: none;
  margin: var(--lr-space-4) 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-2);
}

.lr-onboard__check {
  display: flex;
  align-items: center;
  gap: var(--lr-space-3);
  padding: var(--lr-space-3);
  border-radius: var(--lr-radius-md);
  background: var(--lr-bg-subtle);
}

.lr-onboard__state {
  display: inline-flex;
  flex: none;
}

.lr-onboard__state.is-ok {
  color: var(--lr-success);
}
.lr-onboard__state.is-warn {
  color: var(--lr-warning);
}
.lr-onboard__state.is-error {
  color: var(--lr-danger);
}
.lr-onboard__state.is-pending {
  color: var(--lr-text-tertiary);
}

.lr-onboard__checkmain {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.lr-onboard__checklabel {
  font-size: var(--lr-fs-body);
}

.lr-onboard__checkdetail {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-onboard__fix {
  flex: none;
  height: 26px;
  padding: 0 var(--lr-space-3);
  font-size: var(--lr-fs-secondary);
}

/* ---- 登录 ---- */
.lr-onboard__device {
  margin-top: var(--lr-space-4);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--lr-space-3);
  padding: var(--lr-space-5);
  border-radius: var(--lr-radius-lg);
  background: var(--lr-bg-subtle);
}

.lr-onboard__device--failed {
  background: var(--lr-danger-soft);
}

.lr-onboard__devlabel {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
}

.lr-onboard__code {
  font-family: var(--lr-font-mono);
  font-size: 26px;
  letter-spacing: 3px;
  color: var(--lr-text);
}

.lr-onboard__wait {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-onboard__spin {
  animation: lr-spin 0.9s linear infinite;
}

@keyframes lr-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .lr-onboard__spin {
    animation: none;
  }
}

.lr-onboard__url {
  font-size: var(--lr-fs-mono);
  color: var(--lr-text-tertiary);
  word-break: break-all;
  text-align: center;
}

.lr-onboard__done {
  margin-top: var(--lr-space-4);
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  padding: var(--lr-space-3) var(--lr-space-4);
  border-radius: var(--lr-radius-md);
  background: var(--lr-success-soft);
  border: 0.5px solid var(--lr-success-border);
  font-size: var(--lr-fs-body);
}

.lr-onboard__doneicon {
  color: var(--lr-success);
}

/* ---- 输出目录 ---- */
.lr-onboard__dir {
  margin-top: var(--lr-space-4);
}

/* ---- 完成 ---- */
.lr-onboard__finish {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: var(--lr-space-2);
  padding: var(--lr-space-5) 0;
}

.lr-onboard__finishicon {
  width: 48px;
  height: 48px;
  border-radius: 50%;
  background: var(--lr-success-soft);
  color: var(--lr-success);
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: var(--lr-space-2);
}

.lr-onboard__inlinepath {
  font-family: var(--lr-font-mono);
  font-size: var(--lr-fs-mono);
  color: var(--lr-text-secondary);
  word-break: break-all;
}

/* ---- 底部 ---- */
.lr-onboard__foot {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  margin-top: var(--lr-space-5);
  padding-top: var(--lr-space-4);
  border-top: 0.5px solid var(--lr-border);
}

.lr-onboard__spacer {
  flex: 1;
}
</style>