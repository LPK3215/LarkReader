// ============================================================================
// src/stores/onboarding.ts —— Onboarding 引导流程状态
//
// 4 步：环境体检 → 登录飞书 → 输出目录 → 完成
//
// 步骤 1：checkEnv 把 EnvStatus 映射成 4 条 CheckItem；缺 lark-cli 时显示"安装"
// 步骤 2：startLogin → 浏览器授权 → completeLogin 轮询（3s 一次，最长 5 分钟）
// 步骤 3：复用 settings store 的 pickDir()，选择完后预检可写性
// 步骤 4：finish() 跳 /workspace
//
// 真机专享：所有动作走 IPC；不再保留浏览器假数据兜底。
// ============================================================================

import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { EnvStatus } from "../api/types";
import {
  checkEnv,
  completeLogin,
  setupLarkCli,
  startLogin,
} from "../api/env";

export type CheckState = "pending" | "ok" | "warn" | "error";

export interface CheckItem {
  key: string;
  label: string;
  detail: string;
  state: CheckState;
  /** 当 state != ok 时显示在右边的修复按钮文案 */
  action?: string;
}

export type LoginState = "idle" | "awaiting" | "done" | "failed";

const POLL_INTERVAL_MS = 3000;
const POLL_TIMEOUT_MS = 5 * 60 * 1000;

export const useOnboardingStore = defineStore("onboarding", () => {
  const step = ref(0);
  const checking = ref(false);

  const checks = ref<CheckItem[]>([]);
  const envReady = computed(() =>
    checks.value.length > 0 && checks.value.every((c) => c.state === "ok")
  );

  const loginState = ref<LoginState>("idle");
  const deviceCode = ref("");
  const verificationUrl = ref("");
  const userName = ref<string | null>(null);
  const loginError = ref<string | null>(null);

  let pollHandle: number | null = null;
  let pollStartedAt = 0;

  /** 把 EnvStatus 翻译成 UI 列表。 */
  function buildChecks(env: EnvStatus): CheckItem[] {
    const out: CheckItem[] = [];

    out.push({
      key: "node",
      label: "Node.js",
      detail: env.node_installed
        ? `v${env.node_version ?? "未知"}`
        : "未检测到 Node.js",
      state: env.node_installed ? "ok" : "error",
      action: env.node_installed ? undefined : "安装指引",
    });

    if (env.node_installed) {
      out.push({
        key: "cli",
        label: "lark-cli",
        detail: env.lark_cli_installed
          ? env.lark_cli_compatible
            ? env.lark_cli_version ?? "已安装"
            : `${env.lark_cli_version ?? ""}（版本不兼容）`
          : "未安装",
        state: env.lark_cli_compatible
          ? "ok"
          : env.lark_cli_installed
            ? "warn"
            : "error",
        action: env.lark_cli_compatible ? undefined : "安装/更新",
      });
    }

    out.push({
      key: "app",
      label: "飞书应用配置",
      detail: env.app_configured ? env.app_id ?? "已配置" : "未配置",
      state: env.app_configured ? "ok" : "error",
    });

    out.push({
      key: "login",
      label: "飞书登录状态",
      detail: env.logged_in
        ? `已登录 · ${env.user_name ?? env.token_status ?? "已授权"}`
        : "未登录",
      state: env.logged_in ? "ok" : "warn",
    });

    return out;
  }

  /** 步骤 1：跑一次完整环境体检。 */
  async function runCheck() {
    checking.value = true;
    try {
      const env = await checkEnv();
      checks.value = buildChecks(env);
      if (env.logged_in) {
        loginState.value = "done";
        userName.value = env.user_name ?? null;
      }
    } catch (err) {
      checks.value = [
        {
          key: "node",
          label: "环境检测失败",
          detail: String(err),
          state: "error",
        },
      ];
    } finally {
      checking.value = false;
    }
  }

  /** 步骤 1：安装/更新 lark-cli，再跑一次体检。 */
  async function installCli() {
    checking.value = true;
    try {
      await setupLarkCli();
      await runCheck();
    } catch (err) {
      loginError.value = String(err);
    } finally {
      checking.value = false;
    }
  }

  /** 步骤 2：发起设备码登录，弹出浏览器授权页。 */
  async function beginLogin() {
    loginError.value = null;
    try {
      const info = await startLogin();
      deviceCode.value = info.device_code;
      verificationUrl.value = info.verification_url;
      loginState.value = "awaiting";
      startPolling();
      try {
        await openUrl(info.verification_url);
      } catch {
        // 用户拒绝了打开外部链接的权限，授权码仍然显示在页面里
      }
    } catch (err) {
      loginError.value = String(err);
      loginState.value = "failed";
    }
  }

  function startPolling() {
    stopPolling();
    pollStartedAt = Date.now();
    pollHandle = window.setInterval(tick, POLL_INTERVAL_MS);
  }

  function stopPolling() {
    if (pollHandle != null) {
      window.clearInterval(pollHandle);
      pollHandle = null;
    }
  }

  async function tick() {
    if (Date.now() - pollStartedAt > POLL_TIMEOUT_MS) {
      stopPolling();
      loginError.value = "授权超时，请在 5 分钟内完成浏览器授权";
      loginState.value = "failed";
      return;
    }
    if (!deviceCode.value) return;
    try {
      const result = await completeLogin(deviceCode.value);
      if (result.success) {
        userName.value = result.user_name;
        loginState.value = "done";
        stopPolling();
      } else if (result.error && result.error !== "PENDING") {
        // PENDING = 还在等用户授权；其他错误视为失败
        loginError.value = result.error;
        loginState.value = "failed";
        stopPolling();
      }
    } catch (err) {
      // 轮询失败通常为 transient；下一次继续试
      console.warn("[onboarding.completeLogin]", err);
    }
  }

  function cancelLogin() {
    stopPolling();
    loginState.value = "idle";
    deviceCode.value = "";
    verificationUrl.value = "";
  }

  function reset() {
    cancelLogin();
    step.value = 0;
    loginError.value = null;
  }

  return {
    step,
    checking,
    checks,
    envReady,
    loginState,
    deviceCode,
    verificationUrl,
    userName,
    loginError,
    runCheck,
    installCli,
    beginLogin,
    cancelLogin,
    reset,
  };
});