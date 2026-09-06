// ============================================================================
// src/router/index.ts —— 视图路由表
//
//   /onboarding  环境体检 / 登录 / 默认输出目录引导，全屏无壳（meta.bare）
//   /workspace   主工作台：贴链接 -> 扫树勾选 -> 下载
//   /history     任务历史 + 打开产物目录
//   /terminal    飞书终端：手动体检环境 / 登录 / 退出 / 切换账号
//   /logs        运行日志：查看/刷新后端持久化的运行日志文件
//   /settings    设置
//
// 说明：Tauri 是本地桌面应用，用 createWebHashHistory 而非 history 模式，
//       避免 file:// 或 tauri:// 协议下刷新路由失效。
// ============================================================================

import { createRouter, createWebHashHistory, type RouteRecordRaw } from "vue-router";

// ---- 首次使用标记：未完成引导的新用户会被送往 /onboarding ----
const ONBOARDED_KEY = "larkreader_onboarded";

export function isOnboarded(): boolean {
  try {
    return localStorage.getItem(ONBOARDED_KEY) === "1";
  } catch {
    return true; // localStorage 不可用时不劫持路由
  }
}

export function markOnboarded(): void {
  try {
    localStorage.setItem(ONBOARDED_KEY, "1");
  } catch {
    /* 忽略 */
  }
}

const routes: RouteRecordRaw[] = [
  {
    path: "/",
    redirect: "/workspace",
  },
  {
    path: "/onboarding",
    name: "onboarding",
    component: () => import("../views/OnboardingView.vue"),
    meta: { bare: true, title: "开始使用" },
  },
  {
    path: "/workspace",
    name: "workspace",
    component: () => import("../views/WorkspaceView.vue"),
    meta: { title: "工作台" },
  },
  {
    path: "/history",
    name: "history",
    component: () => import("../views/HistoryView.vue"),
    meta: { title: "任务历史" },
  },
  {
    path: "/reader",
    name: "reader",
    component: () => import("../views/ReaderView.vue"),
    meta: { title: "本地阅读" },
  },
  {
    path: "/terminal",
    name: "terminal",
    component: () => import("../views/TerminalView.vue"),
    meta: { title: "飞书终端" },
  },
  {
    path: "/logs",
    name: "logs",
    component: () => import("../views/LogView.vue"),
    meta: { title: "运行日志" },
  },
  {
    path: "/settings",
    name: "settings",
    component: () => import("../views/SettingsView.vue"),
    meta: { title: "设置" },
  },
  {
    path: "/:pathMatch(.*)*",
    redirect: "/workspace",
  },
];

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
});

// 首次使用（未完成引导）时，除引导页外的任何路由都先送往 /onboarding。
// 完成引导或跳过后写入标记，之后不再拦截。
router.beforeEach((to) => {
  if (isOnboarded() || to.path === "/onboarding") return true;
  return "/onboarding";
});

export default router;
