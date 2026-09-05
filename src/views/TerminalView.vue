<script setup lang="ts">
// ============================================================================
// TerminalView —— 飞书终端管理页
//
// 运行期维护面板：手动体检环境（Node / lark-cli / 应用配置 / 登录状态）、
// 手动登录飞书（设备码流）、退出登录 / 切换账号。
// 与右上角状态胶囊共用 stores/auth.ts，状态实时一致；动作全部走 IPC
// （api/env.ts），不保留浏览器假数据。
//
// 登录模型与 onboarding 一致：start_login 拿设备码 -> 浏览器授权 ->
// complete_login 单次阻塞等待授权完成（勿并发轮询）。
// ============================================================================

import { computed, onMounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { EnvStatus } from "../api/types";
import { useAuthStore } from "../stores/auth";
import AppIcon from "../components/AppIcon.vue";
import AppConfigGuide from "../components/AppConfigGuide.vue";
import { dialog, message } from "../composables/useMessage";

const auth = useAuthStore();
const showAppGuide = ref(false);

type RowState = "ok" | "warn" | "error";

interface Row {
  key: string;
  label: string;
  detail: string;
  state: RowState;
  /** 当行右侧出现修复按钮的文案（cli -> 安装/更新，app 未配置 -> 去创建） */
  action?: string;
}

const ICON: Record<RowState, string> = {
  ok: "check-circle",
  warn: "alert-circle",
  error: "close-circle",
};

/** 把 env 翻译成环境卡里的逐项状态（与 onboarding 页面同口径）。 */
function buildRows(env: EnvStatus): Row[] {
  const out: Row[] = [];

  out.push({
    key: "node",
    label: "Node.js",
    detail: env.node_installed ? `v${env.node_version ?? "未知"}` : "未检测到 Node.js",
    state: env.node_installed ? "ok" : "error",
  });

  out.push({
    key: "cli",
    label: "lark-cli",
    detail: env.lark_cli_installed
      ? env.lark_cli_compatible
        ? env.lark_cli_version ?? "已安装"
        : `${env.lark_cli_version ?? ""}（版本不兼容）`
      : "未安装",
    state: env.lark_cli_compatible ? "ok" : env.lark_cli_installed ? "warn" : "error",
    action: env.lark_cli_compatible ? undefined : "安装/更新",
  });

  out.push({
    key: "app",
    label: "飞书应用配置",
    detail: env.app_configured ? env.app_id ?? "已配置" : "未配置",
    state: env.app_configured ? "ok" : "error",
    action: env.app_configured ? undefined : "去创建",
  });

  const needRefresh = env.logged_in && env.token_status === "needs_refresh";
  out.push({
    key: "login",
    label: "飞书登录状态",
    detail: env.logged_in
      ? `已登录 · ${env.user_name ?? "已授权"}${needRefresh ? "（待刷新）" : ""}`
      : "未登录",
    state: env.logged_in ? (needRefresh ? "warn" : "ok") : "warn",
  });

  return out;
}

const rows = computed<Row[]>(() => (auth.env ? buildRows(auth.env) : []));

/** check_errors 里版本兼容性已并入 cli 行，这里只展示其余体检异常。 */
const extraErrors = computed(
  () => (auth.env?.check_errors ?? []).filter((e) => e.component !== "lark_cli_version")
);

const tokenLabel = computed(() => {
  const t = auth.tokenStatus;
  return t === "ready" ? "凭据有效" : t === "needs_refresh" ? "凭据待刷新" : "无凭据";
});

/**
 * 登录刚完成、env 还在刷新同步时（loggedIn 尚未翻转），先把账号卡显示出来，
 * 避免成功瞬间闪回「开始登录」又跳成已登录。
 */
const accountShown = computed(() => auth.loggedIn || auth.loginState === "done");
const accountSub = computed(() =>
  auth.loggedIn ? tokenLabel.value : "登录已确认，同步状态中…"
);

/** 进入页面立即体检一次，保证看到的是最新状态。 */
onMounted(() => {
  void auth.refresh();
});

async function onRefresh() {
  await auth.refresh();
}

async function onFixCli() {
  try {
    await auth.installCli();
    message.success("lark-cli 已就绪");
  } catch (err) {
    message.error(String(err));
  }
}

async function onBeginLogin() {
  await auth.beginLogin();
}

function cancelLogin() {
  auth.cancelLogin();
}

async function openVerificationUrl() {
  if (!auth.verificationUrl) return;
  try {
    await openUrl(auth.verificationUrl);
  } catch {
    // 打开外部链接被拒绝时，设备码仍显示在页面上
  }
}

function onLogout() {
  dialog.warning({
    title: "退出飞书登录",
    content: `确定退出账号「${auth.userName ?? ""}」吗？退出后需要重新在浏览器授权才能继续导出文档，已导出的本地文件不受影响。`,
    positiveText: "退出登录",
    onPositiveClick: async () => {
      try {
        await auth.logout();
        message.success("已退出飞书登录");
      } catch (err) {
        message.error(String(err));
      }
    },
  });
}

function onSwitchAccount() {
  dialog.warning({
    title: "切换飞书账号",
    content: "将先退出当前登录，再发起新的飞书授权。确定继续吗？",
    positiveText: "退出并登录新账号",
    onPositiveClick: async () => {
      try {
        await auth.logout();
        await auth.beginLogin();
      } catch (err) {
        message.error(String(err));
      }
    },
  });
}
</script>

<template>
  <div class="lr-page">
    <header class="lr-page__head">
      <h1 class="lr-page__title">飞书终端</h1>
      <p class="lr-page__desc">
        检查运行环境与飞书连接状态；登录、退出或切换账号都在这里手动完成
      </p>
    </header>

    <div class="lr-page__body lr-term">
      <!-- 账号卡 -->
      <section class="lr-card">
        <header class="lr-card__head">
          <span class="lr-card__title">飞书账号</span>
          <span class="lr-card__meta">{{ accountShown ? "已登录" : "未登录" }}</span>
        </header>
        <div class="lr-card__body">
          <!-- 已登录（或刚授权完成、env 正在同步） -->
          <template v-if="accountShown">
            <div class="lr-term__account">
              <span class="lr-term__avatar">
                <AppIcon name="user" :size="16" />
              </span>
              <span class="lr-term__accountmain">
                <span class="lr-term__accountname lr-selectable">
                  {{ auth.userName || "已授权账号" }}
                </span>
                <span class="lr-term__accountsub">{{ accountSub }}</span>
              </span>
            </div>
            <p class="lr-term__tip">
              凭据由 lark-cli 本地保管；退出后再次导出文档前需要重新授权。
            </p>
            <div class="lr-term__actions">
              <button class="lr-btn lr-btn--secondary" @click="onSwitchAccount">
                切换账号
              </button>
              <button
                class="lr-btn lr-btn--danger"
                :disabled="auth.loggingOut"
                @click="onLogout"
              >
                {{ auth.loggingOut ? "退出中…" : "退出登录" }}
              </button>
            </div>
          </template>

          <!-- 等待授权 -->
          <div v-else-if="auth.loginState === 'awaiting'" class="lr-term__device">
            <p class="lr-term__devlabel">在浏览器打开链接并输入设备码完成授权</p>
            <code class="lr-term__code lr-selectable">{{ auth.deviceCode }}</code>
            <button class="lr-btn lr-btn--primary lr-btn--lg" @click="openVerificationUrl">
              <AppIcon name="external" :size="14" />
              打开浏览器授权
            </button>
            <p class="lr-term__wait">
              <AppIcon name="spinner" :size="12" class="lr-icon-spin" />
              等待授权完成…
            </p>
            <code class="lr-term__url lr-selectable">{{ auth.verificationUrl }}</code>
            <button class="lr-btn lr-btn--ghost" @click="cancelLogin">取消</button>
          </div>

          <!-- 登录失败 -->
          <div v-else-if="auth.loginState === 'failed'" class="lr-term__device lr-term__device--failed">
            <p class="lr-term__devlabel">登录失败</p>
            <p class="lr-term__devdesc">{{ auth.loginError }}</p>
            <button class="lr-btn lr-btn--primary" @click="onBeginLogin">重试</button>
          </div>

          <!-- 未登录 -->
          <div v-else class="lr-term__device">
            <p class="lr-term__devlabel">使用设备码登录，本应用不接触你的账号密码</p>
            <button class="lr-btn lr-btn--primary lr-btn--lg" @click="onBeginLogin">
              <AppIcon name="external" :size="14" />
              登录飞书
            </button>
          </div>
        </div>
      </section>

      <!-- 环境卡 -->
      <section class="lr-card">
        <header class="lr-card__head">
          <span class="lr-card__title">运行环境</span>
          <button
            class="lr-btn lr-btn--secondary lr-term__recheck"
            :disabled="auth.refreshing"
            @click="onRefresh"
          >
            <AppIcon v-if="auth.refreshing" name="spinner" :size="12" class="lr-icon-spin" />
            {{ auth.refreshing ? "检测中…" : "重新检测" }}
          </button>
        </header>
        <div class="lr-card__body">
          <template v-if="rows.length > 0">
            <ul class="lr-term__checks">
              <li v-for="row in rows" :key="row.key" class="lr-term__check">
                <span class="lr-term__state" :class="`is-${row.state}`">
                  <AppIcon :name="ICON[row.state]" :size="14" />
                </span>
                <span class="lr-term__checkmain">
                  <span class="lr-term__checklabel">{{ row.label }}</span>
                  <span class="lr-term__checkdetail">{{ row.detail }}</span>
                </span>
                <button
                  v-if="row.action"
                  class="lr-btn lr-btn--secondary lr-term__fix"
                  :disabled="auth.refreshing"
                  @click="row.key === 'app' ? (showAppGuide = true) : onFixCli()"
                >
                  {{ row.action }}
                </button>
              </li>
            </ul>

            <AppConfigGuide
              v-if="showAppGuide"
              :busy="auth.refreshing"
              @close="showAppGuide = false"
              @recheck="onRefresh"
            />
          </template>

          <p v-else class="lr-term__hint">
            {{ auth.refreshing ? "正在检测运行环境…" : "尚未检测，点击右上角「重新检测」" }}
          </p>

          <div v-if="extraErrors.length > 0" class="lr-term__errors">
            <p v-for="(e, i) in extraErrors" :key="i" class="lr-term__error">
              <AppIcon name="alert-circle" :size="13" />
              {{ e.component }}：{{ e.message }}
            </p>
          </div>
          <p v-if="auth.envError" class="lr-term__errors">
            <AppIcon name="alert-circle" :size="13" />
            体检失败：{{ auth.envError }}
          </p>
        </div>
      </section>

      <!-- 图例说明 -->
      <section class="lr-card lr-term__legend">
        <div class="lr-card__body lr-term__legendbody">
          <span class="lr-term__legenddot is-ready" />环境正常，可直接导出；
          <span class="lr-term__legenddot is-warn" />有未登录或版本等提醒；
          <span class="lr-term__legenddot is-error" />缺依赖需先处理。
          右上角状态胶囊与这里状态一致，点击胶囊可直达本页。
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.lr-term {
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-4);
  padding-right: var(--lr-space-1);
}

/* ---- 账号 ---- */
.lr-term__account {
  display: flex;
  align-items: center;
  gap: var(--lr-space-3);
}

.lr-term__avatar {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  flex: none;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--lr-primary-soft);
  color: var(--lr-primary);
}

.lr-term__accountmain {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.lr-term__accountname {
  font-size: var(--lr-fs-body);
  font-weight: var(--lr-fw-medium);
}

.lr-term__accountsub {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-term__tip {
  margin-top: var(--lr-space-3);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
  line-height: var(--lr-lh-body);
}

.lr-term__actions {
  margin-top: var(--lr-space-4);
  display: flex;
  gap: var(--lr-space-2);
}

/* ---- 设备码登录 ---- */
.lr-term__device {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--lr-space-3);
  padding: var(--lr-space-4);
  border-radius: var(--lr-radius-lg);
  background: var(--lr-bg-subtle);
}

.lr-term__device--failed {
  background: var(--lr-danger-soft);
}

.lr-term__devlabel {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
}

.lr-term__devdesc {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-danger);
  word-break: break-all;
  text-align: center;
}

.lr-term__code {
  font-family: var(--lr-font-mono);
  font-size: 24px;
  letter-spacing: 3px;
  color: var(--lr-text);
}

.lr-term__wait {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-term__url {
  font-size: var(--lr-fs-mono);
  color: var(--lr-text-tertiary);
  word-break: break-all;
  text-align: center;
}

/* ---- 环境检查 ---- */
.lr-term__recheck {
  height: 26px;
  padding: 0 var(--lr-space-3);
  font-size: var(--lr-fs-secondary);
}

.lr-term__checks {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-2);
}

.lr-term__check {
  display: flex;
  align-items: center;
  gap: var(--lr-space-3);
  padding: var(--lr-space-3);
  border-radius: var(--lr-radius-md);
  background: var(--lr-bg-subtle);
}

.lr-term__state {
  display: inline-flex;
  flex: none;
}

.lr-term__state.is-ok {
  color: var(--lr-success);
}

.lr-term__state.is-warn {
  color: var(--lr-warning);
}

.lr-term__state.is-error {
  color: var(--lr-danger);
}

.lr-term__checkmain {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.lr-term__checklabel {
  font-size: var(--lr-fs-body);
}

.lr-term__checkdetail {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
  word-break: break-all;
}

.lr-term__fix {
  flex: none;
  height: 26px;
  padding: 0 var(--lr-space-3);
  font-size: var(--lr-fs-secondary);
}

.lr-term__hint {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-term__errors {
  margin-top: var(--lr-space-3);
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-1);
  padding: var(--lr-space-3) var(--lr-space-4);
  border-radius: var(--lr-radius-md);
  background: var(--lr-warning-soft);
  border: 0.5px solid var(--lr-warning-border);
  color: var(--lr-warning);
  font-size: var(--lr-fs-secondary);
}

.lr-term__error {
  display: flex;
  align-items: flex-start;
  gap: var(--lr-space-2);
  word-break: break-all;
}

/* ---- 图例 ---- */
.lr-term__legend {
  background: transparent;
  border-style: dashed;
}

.lr-term__legendbody {
  display: flex;
  align-items: center;
  gap: var(--lr-space-1);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
  line-height: var(--lr-lh-body);
  flex-wrap: wrap;
}

.lr-term__legenddot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
  margin: 0 var(--lr-space-1) 0 var(--lr-space-2);
}

.lr-term__legenddot.is-ready {
  background: var(--lr-success);
}

.lr-term__legenddot.is-warn {
  background: var(--lr-warning);
}

.lr-term__legenddot.is-error {
  background: var(--lr-danger);
}
</style>
