<script setup lang="ts">
// ============================================================================
// OnboardingView —— 环境体检 / 登录 / 默认输出目录引导
//
// 全屏无壳（路由 meta.bare），完成即跳转 /workspace，不再出现。
// 流程：check_env ->（缺 lark-cli 则 setup_lark_cli）-> 登录 -> 选默认输出目录
//
// 三个硬前置必须在进入工作台前解决：
//   1. Node.js + lark-cli 可用     2. 飞书已登录     3. 有可写的默认输出目录
// 登录走设备码流（start_login 拿设备码 -> 浏览器授权 -> complete_login 单次阻塞等待授权）。
// 注意：不要对 complete_login 做并发轮询——lark-cli 每次重启都会作废上一轮的 device code。
// 真机专享：所有动作走 IPC；不再保留浏览器假数据兜底。
// ============================================================================

import { onBeforeUnmount, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useSettingsStore } from "../stores/settings";
import { useOnboardingStore, type CheckState } from "../stores/onboarding";
import { openUrl } from "@tauri-apps/plugin-opener";
import DirPicker from "../components/DirPicker.vue";
import AppIcon from "../components/AppIcon.vue";
import AppConfigGuide from "../components/AppConfigGuide.vue";
import { dialog, message } from "../composables/useMessage";
import { markOnboarded } from "../router";

const router = useRouter();
const settings = useSettingsStore();
const onboarding = useOnboardingStore();

// ---- 首次使用合规确认：勾选同意后才进入引导，之后不再显示 ----
const AGREEMENT_KEY = "larkreader_agreement_v1";
const agreed = ref((() => {
  try {
    return localStorage.getItem(AGREEMENT_KEY) === "1";
  } catch {
    return true;
  }
})());
const agreeChecked = ref(false);

function acceptAgreement() {
  if (!agreeChecked.value) return;
  try {
    localStorage.setItem(AGREEMENT_KEY, "1");
  } catch {
    /* 忽略 */
  }
  agreed.value = true;
}

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

const showAppGuide = ref(false);
const recheckBusy = ref(false);
async function recheck() {
  recheckBusy.value = true;
  try {
    await onboarding.runCheck();
  } finally {
    recheckBusy.value = false;
  }
}

// ---- lark-cli 安装方式：自动（带进度）/ 手动（复制命令去终端） ----
// 命令与后端 SUPPORTED_LARK_CLI_VERSION 保持一致
const INSTALL_CMD = "npm install -g @larksuite/cli@1.0.93";

/** null=收起 choose=二选一 auto=自动安装进行中 */
const cliPanel = ref<null | "choose" | "auto">(null);

function askInstallCli() {
  cliPanel.value = "choose";
}

async function autoInstallCli() {
  cliPanel.value = "auto";
  await onboarding.installCli();
  cliPanel.value = null;
}

async function manualInstallCli() {
  try {
    await navigator.clipboard.writeText(INSTALL_CMD);
    message.success("安装命令已复制，去终端运行后点「重新检测」");
  } catch {
    message.warning(`复制失败，请手动输入：${INSTALL_CMD}`);
  }
  cliPanel.value = null;
}

/** 步骤 3 离开时把「默认输出目录」落盘，避免重启后丢失选择 */
const savingDir = ref(false);
async function onNextStep() {
  if (onboarding.step === 2) {
    if (savingDir.value) return;
    savingDir.value = true;
    let saved = false;
    try {
      await settings.save();
      saved = true;
    } catch (err) {
      // 目录不可写等：warning 已同步并全局提示，停在当前步让用户处理
      console.warn("[onboarding] 保存输出目录失败:", err);
    } finally {
      savingDir.value = false;
    }
    if (saved) nextStep();
    return;
  }
  nextStep();
}

/** 选目录后立即预检，进入工作台时也保证 settings.save 落盘 */
function onDirPick(path: string) {
  void settings.refreshPreflight(path);
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

/** 登录环节复制设备码 / 授权链接：自动打开失败时用户可自行处理 */
async function copyLoginValue(kind: "code" | "url") {
  const text = kind === "code" ? onboarding.deviceCode : onboarding.verificationUrl;
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
    message.success(kind === "code" ? "设备码已复制" : "链接已复制");
  } catch {
    message.warning("复制失败，请手动选择复制");
  }
}

function nextStep() {
  onboarding.step = onboarding.step + 1;
}
function prevStep() {
  onboarding.step = Math.max(0, onboarding.step - 1);
}

/** 完成引导：写入标记，之后启动不再自动弹出 */
function finish() {
  markOnboarded();
  router.push("/workspace");
}

/** 跳过引导：确认后写入标记，本次及以后启动都不再自动弹出 */
function skipOnboarding() {
  dialog.warning({
    title: "跳过引导",
    content: "跳过后启动时将不再自动弹出引导，仍可随时前往「飞书终端」手动配置环境。确定跳过吗？",
    positiveText: "跳过引导",
    onPositiveClick: () => {
      markOnboarded();
      router.push("/workspace");
    },
  });
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
      <template v-if="agreed">
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
            导出依赖 Node.js 与固定版本的 lark-cli，缺项可自动安装或复制命令手动安装
          </p>

          <template v-if="onboarding.checks.length > 0">
            <ul class="lr-onboard__checks">
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
                  :disabled="onboarding.checking || onboarding.installing"
                  @click="askInstallCli"
                >
                  {{ item.action }}
                </button>
                <button
                  v-else-if="item.action && item.key === 'app'"
                  class="lr-btn lr-btn--secondary lr-onboard__fix"
                  :disabled="onboarding.checking"
                  @click="showAppGuide = true"
                >
                  {{ item.action }}
                </button>
                <button
                  v-else-if="item.action && item.key === 'login'"
                  class="lr-btn lr-btn--secondary lr-onboard__fix"
                  @click="onboarding.step = 1"
                >
                  {{ item.action }}
                </button>
              </li>
            </ul>

            <!-- lark-cli 安装方式：自动（带进度）/ 手动（复制命令） -->
            <div v-if="cliPanel" class="lr-onboard__clipanel">
              <template v-if="cliPanel === 'choose'">
                <div class="lr-onboard__cliopts">
                  <button
                    class="lr-btn lr-btn--primary"
                    :disabled="onboarding.installing"
                    @click="autoInstallCli"
                  >
                    <AppIcon name="download" :size="13" />
                    自动安装
                  </button>
                  <button class="lr-btn lr-btn--secondary" @click="manualInstallCli">
                    复制命令，手动安装
                  </button>
                </div>
                <p class="lr-onboard__clihint">自动安装约需几十秒，完成后自动重新检测</p>
              </template>
              <p v-else class="lr-onboard__clititle">
                <AppIcon name="spinner" :size="13" class="lr-icon-spin" />
                正在安装 lark-cli…
              </p>
            </div>

            <AppConfigGuide
              v-if="showAppGuide"
              :busy="onboarding.checking"
              @close="showAppGuide = false"
              @recheck="recheck"
            />
          </template>

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
            <div class="lr-onboard__devops">
              <button class="lr-btn lr-btn--primary" @click="openVerificationUrl">
                <AppIcon name="external" :size="14" />
                打开浏览器授权
              </button>
              <button class="lr-btn lr-btn--secondary" @click="copyLoginValue('code')">
                复制设备码
              </button>
            </div>
            <p class="lr-onboard__wait">
              <AppIcon name="spinner" :size="12" class="lr-icon-spin" />
              等待授权完成…
            </p>
            <code class="lr-onboard__url lr-selectable">{{ onboarding.verificationUrl }}</code>
            <div class="lr-onboard__devops">
              <button class="lr-btn lr-btn--ghost" @click="copyLoginValue('url')">
                复制链接
              </button>
              <button class="lr-btn lr-btn--ghost" @click="cancelLogin">取消</button>
            </div>
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
              @pick="onDirPick"
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
        <button
          v-if="onboarding.step === 0"
          class="lr-btn lr-btn--ghost"
          @click="skipOnboarding"
        >
          跳过引导
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
            savingDir ||
            (onboarding.step === 0 && !onboarding.envReady) ||
            (onboarding.step === 1 && onboarding.loginState !== 'done')
          "
          @click="onNextStep"
        >
          {{ savingDir ? "保存中…" : "下一步" }}
        </button>
      </footer>
      </template>

      <!-- 首次使用：合规确认 -->
      <template v-else>
        <h2 class="lr-onboard__title">使用前请阅读</h2>
        <div class="lr-onboard__agreement">
          <ul>
            <li>本工具通过飞书<strong>官方 Open API</strong> 与官方 lark-cli，以你自己的账号授权读取你<strong>有权限查看</strong>的内容；不破解、不绕过任何权限机制。</li>
            <li>导出内容的<strong>版权归原作者所有</strong>，仅限个人学习与备份，请勿用于传播或商业用途。</li>
            <li>请遵守飞书用户协议、开放平台条款及所在组织的数据安全规定；因违规使用产生的后果由使用者自行承担。</li>
            <li>本项目为学习交流项目，与飞书 / 字节跳动官方无关联。</li>
          </ul>
          <label class="lr-onboard__agreelabel">
            <input v-model="agreeChecked" type="checkbox" />
            <span>我已阅读并同意以上声明</span>
          </label>
          <button
            class="lr-btn lr-btn--primary lr-btn--lg"
            :disabled="!agreeChecked"
            @click="acceptAgreement"
          >
            同意并继续
          </button>
        </div>
      </template>
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
  max-height: calc(100vh - 48px);
  overflow: hidden;
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

/* ---- 合规确认 ---- */
.lr-onboard__agreement {
  margin-top: var(--lr-space-4);
  padding: var(--lr-space-4);
  border: 0.5px solid var(--lr-border);
  border-radius: var(--lr-radius-lg);
  background: var(--lr-bg-subtle);
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-3);
}

.lr-onboard__agreement ul {
  margin: 0;
  padding-left: var(--lr-space-4);
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
  line-height: var(--lr-lh-body);
}

.lr-onboard__agreelabel {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-body);
  color: var(--lr-text);
  cursor: pointer;
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

/* ---- 内容区：独立滚动，底栏固定不遮挡 ---- */
.lr-onboard__body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
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

/* ---- lark-cli 安装方式面板 ---- */
.lr-onboard__clipanel {
  margin-top: var(--lr-space-3);
  padding: var(--lr-space-3) var(--lr-space-4);
  border: 0.5px dashed var(--lr-border);
  border-radius: var(--lr-radius-md);
  background: var(--lr-bg-subtle);
}

.lr-onboard__cliopts {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
}

.lr-onboard__clihint,
.lr-onboard__clititle {
  margin: var(--lr-space-2) 0 0;
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
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
  font-size: 18px;
  letter-spacing: 2px;
  color: var(--lr-text);
  word-break: break-all;
  max-width: 100%;
}

.lr-onboard__wait {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-onboard__url {
  max-width: 100%;
  word-break: break-all;
  text-align: center;
  font-size: var(--lr-fs-mono);
  color: var(--lr-text-tertiary);
}

.lr-onboard__devops {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  flex-wrap: wrap;
  justify-content: center;
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
  flex: none;
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