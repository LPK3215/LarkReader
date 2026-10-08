// ============================================================================
// src/stores/onboarding.ts —— Onboarding 引导流程状态
//
// 4 步：环境体检 → 登录飞书 → 输出目录 → 完成
//
// 步骤 1：checkEnv 把 EnvStatus 映射成 4 条 CheckItem；缺 lark-cli 时显示"安装"
// 步骤 2：startLogin 拿设备码 → 二维码/链接待用户授权 → completeLogin 单次阻塞等待授权完成
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
  openIsolatedBrowser,
  qrSvg as qrSvgIpc,
  setupLarkCli,
  startLogin,
} from "../api/env";
import { errMsg } from "../composables/useMessage";

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
  /** 授权链接的二维码 SVG 源码（后端本地渲染，链接一换就重画） */
  const qrMarkup = ref("");
  /** 后端回报的"用了哪个浏览器打开授权页"说明，直接展示，避免用户猜窗口是哪来的 */
  const browserNote = ref("");
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
          detail: errMsg(err),
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
          detail: errMsg(err),
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
    browserNote.value = ""; // 清掉上一次的"用哪个浏览器打开"提示
    const seq = ++loginSeq; // 每次发起都作废此前未结束的等待会话
    try {
      const info = await startLogin();
      if (seq !== loginSeq) return;
      deviceCode.value = info.device_code;
      verificationUrl.value = info.verification_url;
      loginState.value = "awaiting";
      // 与飞书终端页同一策略：自动用隔离浏览器打开授权页 + 同时给出二维码；
      // 后续扫码登录与点「开通并授权」由用户自己完成（2026-10-09 实测）。
      void loadQr(info.verification_url);
      void openVerification();
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
      loginError.value = errMsg(err);
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
    qrMarkup.value = "";
    browserNote.value = "";
  }

  /** 渲染授权链接的二维码；失败不影响"链接"这条路，静默留空即可。 */
  async function loadQr(url: string) {
    try {
      qrMarkup.value = await qrSvgIpc(url);
    } catch {
      qrMarkup.value = "";
    }
  }

  /**
   * 用**隔离浏览器**打开授权链接（发起登录时自动调用一次，按钮也可手动触发）。
   * 隔离 profile 的干净实例是本机实测唯一能走通「开通并授权」的环境；
   * 兜底退回系统默认浏览器，再失败就交给面板上的二维码/链接。
   */
  async function openVerification() {
    if (!verificationUrl.value) return;
    try {
      // 后端会回报"用的是哪个浏览器、是否复用了已开窗口"，直接展示给用户：
      // 出问题时能一眼判断窗口是谁开的，不必靠猜（2026-10-09 加固）。
      browserNote.value = await openIsolatedBrowser(verificationUrl.value);
    } catch {
      try {
        await openUrl(verificationUrl.value);
        browserNote.value = "已用系统默认浏览器打开授权页（隔离窗口不可用）";
      } catch {
        browserNote.value = "";
        // 两条路都不行：二维码与链接仍在页面上
      }
    }
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
    qrMarkup,
    browserNote,
    userName,
    loginError,
    runCheck,
    installCli,
    beginLogin,
    openVerification,
    cancelLogin,
    reset,
  };
});