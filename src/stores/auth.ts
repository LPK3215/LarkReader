// ============================================================================
// src/stores/auth.ts —— 飞书环境与登录状态（全局状态源）
//
// 定位：应用外壳（右上角状态胶囊）与「飞书终端」页共用的真实状态源。
// onboarding 页保留它自己的引导状态机，本 store 面向运行期：
//   env    : 最近一次 check_env 的结果（登录名 / token / 版本 / 兼容性）
//   overview: 由 env 推导的胶囊语义（ready / warning / error + 文案）
//   actions: refresh()    手动跑一次环境体检
//            installCli() 安装/更新 lark-cli 后重检
//            beginLogin() 设备码登录（start_login -> 二维码/链接待用户授权 -> complete_login 单次阻塞）
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
  openIsolatedBrowser,
  qrSvg as qrSvgIpc,
  setupLarkCli,
  startLogin,
} from "../api/env";
import { errMsg } from "../composables/useMessage";

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
  if (env.scope_check?.state === "missing")
    return { level: "warning", text: "飞书授权待补齐" };
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
  /** lark-cli 自动安装进行中（终端页显示进度态用） */
  const installing = ref(false);

  // ---- 登录 / 退出流程 ----
  const loginState = ref<LoginFlowState>("idle");
  const deviceCode = ref("");
  const verificationUrl = ref("");
  /** 授权链接的二维码 SVG 源码（后端本地渲染，链接一换就重画） */
  const qrMarkup = ref("");
  const loginError = ref<string | null>(null);
  const loggingOut = ref(false);
  /** 登录会话序号：取消后作废在途的 complete_login 响应，防止旧进程覆盖新会话状态 */
  let loginSeq = 0;

  const loggedIn = computed(() => env.value?.logged_in === true);
  const userName = computed(() => env.value?.user_name ?? null);
  const tokenStatus = computed(() => env.value?.token_status ?? null);

  // ---- 授权范围（scope）巡检状态 ----
  /** 巡检状态：ok / missing / unknown / skipped */
  const scopeState = computed(() => env.value?.scope_check?.state ?? "");
  /** 确认缺失的必需 scope（仅 state === "missing" 时非空） */
  const scopeMissing = computed(() => env.value?.scope_check?.missing ?? []);
  /** 面向用户的巡检说明 */
  const scopeMessage = computed(() => env.value?.scope_check?.message ?? "");
  /**
   * 授权范围是否确认不完整——这是唯一会拦截导出的状态。
   * unknown / skipped 一律放行：宁可让业务命令自己报错，也不误拦用户。
   */
  const scopesIncomplete = computed(() => scopeState.value === "missing");

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
      envError.value = errMsg(err);
    } finally {
      refreshing.value = false;
    }
  }

  /** 安装/更新 lark-cli（供终端页「修复」按钮用），装完重检。 */
  async function installCli() {
    if (installing.value) return;
    installing.value = true;
    try {
      await setupLarkCli();
      await refresh();
    } catch (err) {
      envError.value = errMsg(err);
    } finally {
      installing.value = false;
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
      // 双保险：① 自动用**隔离浏览器**（独立 profile 的干净实例）打开授权页——
      // 日常浏览器常见"点开通并授权毫无反应"，隔离实例实测一次成功；
      // ② 同时渲染二维码，用户也可改用手机扫码。
      // 打开后仍需用户自己完成：扫码登录 + 点「开通并授权」。
      void loadQr(info.verification_url);
      void openVerification();
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
      loginError.value = errMsg(err);
      loginState.value = "failed";
    }
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
   *
   * 隔离 profile 的干净实例是本机实测唯一能走通「开通并授权」的环境；
   * 兜底退回系统默认浏览器，再失败就交给面板上的二维码/链接。
   */
  async function openVerification() {
    if (!verificationUrl.value) return;
    try {
      await openIsolatedBrowser(verificationUrl.value);
    } catch {
      try {
        await openUrl(verificationUrl.value);
      } catch {
        // 两条路都不行：二维码与链接仍在页面上，可自行复制处理
      }
    }
  }

  /** 取消登录：仅复位流程 UI，并作废在途的 complete_login 会话。
   *  后端等待授权的阻塞进程最长约 10 分钟后自行超时，其迟到结果会被序号丢弃。 */
  function cancelLogin() {
    loginSeq++;
    loginState.value = "idle";
    deviceCode.value = "";
    verificationUrl.value = "";
    qrMarkup.value = "";
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

  /**
   * 清除本机登录态并重新登录 —— 授权范围缺项的唯一修复入口。
   *
   * 顺序本身就是要义：`auth logout` 先删掉 lark-cli 里保存的旧令牌（旧令牌里没有
   * 后来新增的 scope），随后的 `auth login --scope <LOGIN_SCOPES>` 才会签发新令牌。
   * 只重新登录而不先登出，等于继续用旧令牌，缺项照旧。
   */
  async function resetLogin() {
    await logout();
    await beginLogin();
  }

  return {
    env,
    refreshing,
    installing,
    envError,
    loggedIn,
    userName,
    tokenStatus,
    scopeState,
    scopeMissing,
    scopeMessage,
    scopesIncomplete,
    overview,
    loginState,
    deviceCode,
    verificationUrl,
    qrMarkup,
    loginError,
    loggingOut,
    refresh,
    installCli,
    beginLogin,
    openVerification,
    cancelLogin,
    logout,
    resetLogin,
  };
});
