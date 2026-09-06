// ============================================================================
// src/stores/onboarding.ts —— Onboarding 引导流程状态
//
// 4 步：环境体检 → 登录飞书 → 输出目录 → 完成
//
// 步骤 1：checkEnv 把 EnvStatus 映射成 4 条 CheckItem；缺 lark-cli 时显示"安装"
// 步骤 2：startLogin 拿设备码 → 浏览器授权 → completeLogin 单次阻塞等待授权完成
//         （后端跑 `lark-cli auth login --device-code`，最长约 10 分钟；勿并发轮询）
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

export const useOnboardingStore = defineStore("onboarding", () => {
  const step = ref(0);
  const checking = ref(false);
  /** lark-cli 自动安装进行中（区别于普通体检，用于显示安装进度态） */
  const installing = ref(false);

  const checks = ref<CheckItem[]>([]);
  // 「登录状态」是步骤 2 的独立关卡，不应阻塞步骤 1（登录）入口：
  // 若把登录 warn/error 也算进来，首次使用/重新登录时会永远无法进入登录页。
  const envReady = computed(() => {
    const relevant = checks.value.filter((c) => c.key !== "login");
    return relevant.length > 0 && relevant.every((c) => c.state === "ok");
  });

  const loginState = ref<LoginState>("idle");
  const deviceCode = ref("");
  const verificationUrl = ref("");
  const userName = ref<string | null>(null);
  const loginError = ref<string | null>(null);
  /** 登录会话序号：取消/离开页面后作废在途 complete_login，防旧进程覆盖新会话 */
  let loginSeq = 0;

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
      action: env.app_configured ? undefined : "去创建",
    });

    out.push({
      key: "login",
      label: "飞书登录状态",
      detail: env.logged_in
        ? `已登录 · ${env.user_name ?? env.token_status ?? "已授权"}`
        : "未登录",
      state: env.logged_in ? "ok" : "warn",
      action: env.logged_in ? undefined : "去登录",
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
      } else {
        // 用户已在外部退出登录：把步骤 1 的“已登录”态复位，避免残留旧用户名/状态
        loginState.value = "idle";
        userName.value = null;
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

  /** 步骤 1：安装/更新 lark-cli，再跑一次体检。失败要落在体检列表上可见，而不是写进登录错误。 */
  async function installCli() {
    if (installing.value) return;
    installing.value = true;
    try {
      await setupLarkCli();
      await runCheck();
    } catch (err) {
      checks.value = [
        {
          key: "cli",
          label: "lark-cli 安装失败",
          detail: String(err),
          state: "error",
        },
      ];
    } finally {
      installing.value = false;
    }
  }

  /** 步骤 2：发起设备码登录，弹浏览器授权页，单次阻塞等待授权完成。 */
  async function beginLogin() {
    if (loginState.value === "awaiting") return; // 已在等待授权，防止重复发起
    loginError.value = null;
    const seq = ++loginSeq; // 每次发起都作废此前未结束的等待会话
    try {
      const info = await startLogin();
      if (seq !== loginSeq) return;
      deviceCode.value = info.device_code;
      verificationUrl.value = info.verification_url;
      loginState.value = "awaiting";
      try {
        await openUrl(info.verification_url);
      } catch {
        // 用户拒绝了打开外部链接的权限，授权码仍然显示在页面里
      }
      // 单次阻塞等待授权：后端运行 `lark-cli auth login --device-code <code>`
      // 直到用户在浏览器完成授权（最长约 10 分钟）。不要改成并发轮询——
      // lark-cli 每次重启该命令都会作废上一轮的 device code，并发等于永远无法登录。
      const result = await completeLogin(deviceCode.value);
      // 取消/离开页面/重新登录后，旧进程迟到的结果只对旧设备码有效，一律丢弃，
      // 避免把新会话的 awaiting（或用户在别处的登录态）误判成失败。
      if (seq !== loginSeq) return;
      if (result.success) {
        userName.value = result.user_name;
        loginState.value = "done";
      } else {
        loginError.value = result.error ?? "登录失败，请重试";
        loginState.value = "failed";
      }
    } catch (err) {
      if (seq !== loginSeq) return;
      loginError.value = String(err);
      loginState.value = "failed";
    }
  }

  /** 取消登录：仅复位 UI 状态并作废在途会话。
   *
   * 后端等待授权的阻塞进程无法中途终止，最长约 10 分钟后自行超时退出，
   * 其迟到结果会被登录序号丢弃，不会影响新会话。
   */
  function cancelLogin() {
    loginSeq++;
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
    installing,
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