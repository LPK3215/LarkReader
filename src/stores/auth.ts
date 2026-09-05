// ============================================================================
// src/stores/auth.ts —— 登录 / 环境状态（结构占位）
//
// 职责（M1 装 pinia 后填充，defineStore('auth', ...)）：
//   state : envStatus(来自 api/env.ts check_env) / userName / 登录流程状态
//   actions: bootstrap()  启动时 check_env，据此决定去 onboarding 还是 workspace
//            login()      驱动 api/auth.ts 两步或阻塞登录
//            logout()     （如后端提供再接入）
//
// 说明：当前不 import pinia（依赖未装，避免破基线）。
// ============================================================================

export {};
