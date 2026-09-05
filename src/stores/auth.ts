// ============================================================================
// src/stores/auth.ts —— 飞书环境与登录状态（全局状态源）
//
// 定位：应用外壳（右上角状态胶囊）与「飞书终端」页共用的真实状态源。
// onboarding 页保留它自己的引导状态机，本 store 面向运行期：
//   env    : 最近一次 check_env 的结果（登录名 / token / 版本 / 兼容性）
//   overview: 由 env 推导的胶囊语义（ready / warning / error + 文案）
//   actions: refresh()    手动跑一次环境体检
//            installCli() 安装/更新 lark-cli 后重检
//            beginLogin() 设备码登录（start_login -> 开浏览器 -> complete_login 单次阻塞）
//            logout()     退出登录（lark-cli auth logout）后重检
//
// 真机专享：所有动作走 IPC；不做浏览器假数据兜底。
// ============================================================================

import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { EnvStatus } from "../api/types";
import {
  checkEnv,
  completeLogin,
  logout as logoutIpc,
  setupLarkCli,
  startLogin,
} from "../api/env";

export type EnvLevel = "ready" | "warning" | "error";

export interface EnvOverview {
  level: EnvLevel;
  text: string;
}

/** 把 env 体检结果压成顶栏胶囊语义。等级取最严重的未达标项。 */
export function describeEnv(
  env: EnvStatus | null,
  error: string | null
): EnvOverview {
  if (error) return { level: "error", text: "环境检测失败" };
  if (!env) return { level: "warning", text: "检测中…" };
  if (!env.node_installed) return { level: "error", text: "Node.js 未安装" };
  if (!env.lark_cli_installed) return { level: "error", text: "lark-cli 未安装" };
  if (!env.lark_cli_compatible)
    return { level: "warning", text: "lark-cli 版本需更新" };
  if (!env.app_configured) return { level: "error", text: "飞书应用未配置" };
  if (!env.logged_in) return { level: "warning", text: "未登录飞书" };
  if (env.token_status === "needs_refresh")
    return { level: "warning", text: "飞书登录待刷新" };
  return { level: "ready", text: "环境正常" };
}

export type LoginFlowState = "idle" | "awaiting" | "done" | "failed";

export const useAuthStore = defineStore("auth", () => {
  // ---- 环境体检 ----
  const env = ref<EnvStatus | null>(null);
  const refreshing = ref(false);
  const envError = ref<string | null>(null);

  // ---- 登录 / 退出流程 ----
  const loginState = ref<LoginFlowState>("idle");
  const deviceCode = ref("");
  const verificationUrl = ref("");
  const loginError = ref<string | null>(null);
  const loggingOut = ref(false);
  /** 登录会话序号：取消后作废在途的 complete_login 响应，防止旧进程覆盖新会话状态 */
  let loginSeq = 0;

  const loggedIn = computed(() => env.value?.logged_in === true);
  const userName = computed(() => env.value?.user_name ?? null);
  const tokenStatus = computed(() => env.value?.token_status ?? null);

  const overview = computed<EnvOverview>(() => {
    if (refreshing.value && !env.value)
      return { level: "warning", text: "检测中…" };
    return describeEnv(env.value, envError.value);
  });

  /** 跑一次完整环境体检，刷新 env。失败不清空旧值，仅记录 envError。 */
  async function refresh() {
    refreshing.value = true;
    try {
      env.value = await checkEnv();
      envError.value = null;
    } catch (err) {
      envError.value = String(err);
    } finally {
      refreshing.value = false;
    }
  }

  /** 安装/更新 lark-cli（供终端页「修复」按钮用），装完重检。 */
  async function installCli() {
    refreshing.value = true;
    try {
      await setupLarkCli();
      await refresh();
    } catch (err) {
      envError.value = String(err);
    } finally {
      refreshing.value = false;
    }
  }

  /**
   * 发起设备码登录：拿设备码 -> 打开浏览器授权 -> 单次阻塞等待授权完成。
   *
   * 不要对 complete_login 做并发轮询——lark-cli 每次重启该命令都会作废
   * 上一轮的 device code，并发等于永远无法登录（与 onboarding 同一模型）。
   */
  async function beginLogin() {
    if (loginState.value === "awaiting") return; // 已在等待授权，防止重复发起
    loginError.value = null;
    const seq = ++loginSeq; // 每次发起都作废此前未结束的等待会话
    try {
      const info = await startLogin();
      if (seq !== loginSeq) return; // 等待期间已被取消/重开
      deviceCode.value = info.device_code;
      verificationUrl.value = info.verification_url;
      loginState.value = "awaiting";
      try {
        await openUrl(info.verification_url);
      } catch {
        // 用户拒绝了打开外部链接的权限，设备码仍然显示在页面里
      }
      const result = await completeLogin(deviceCode.value);
      // 取消后重新登录时旧进程（最长约 10 分钟超时）会迟到返回：
      // 它的结果只对旧设备码有效，一律丢弃，避免覆盖新会话的 awaiting/failed 状态。
      if (seq !== loginSeq) return;
      if (result.success) {
        loginState.value = "done";
        await refresh();
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

  /** 取消登录：仅复位流程 UI，并作废在途的 complete_login 会话。
   *  后端等待授权的阻塞进程最长约 10 分钟后自行超时，其迟到结果会被序号丢弃。 */
  function cancelLogin() {
    loginSeq++;
    loginState.value = "idle";
    deviceCode.value = "";
    verificationUrl.value = "";
  }

  /** 退出登录：清除 lark-cli token 后重检。失败会向上抛出，由调用方提示。 */
  async function logout() {
    if (loggingOut.value) return;
    loggingOut.value = true;
    try {
      cancelLogin();
      await logoutIpc();
      await refresh();
    } finally {
      loggingOut.value = false;
    }
  }

  return {
    env,
    refreshing,
    envError,
    loggedIn,
    userName,
    tokenStatus,
    overview,
    loginState,
    deviceCode,
    verificationUrl,
    loginError,
    loggingOut,
    refresh,
    installCli,
    beginLogin,
    cancelLogin,
    logout,
  };
});
