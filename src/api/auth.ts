// ============================================================================
// src/api/auth.ts —— 登录命令封装（结构占位）
//
// 对应后端命令：
//   start_login()                  -> DeviceInfo { device_code, verification_url }
//   complete_login(device_code)    -> LoginResult  用 device_code 完成登录（轮询调用）
//
// 实际流程：前端「两步非阻塞」= startLogin 拿码/链接 -> 展示 + 打开
//       verificationUrl -> 轮询 completeLogin 直到返回。登录状态进 auth store。
// 真实实现见 ./env.ts 与 stores/auth.ts；此文件保留命令索引，不重复封装。
// ============================================================================

export {};
