// ============================================================================
// src/api/auth.ts —— 登录命令封装（结构占位）
//
// 对应后端命令：
//   start_login()                  -> DeviceInfo { device_code, verification_url }
//   complete_login(device_code)    -> LoginResult  用 device_code 完成登录（轮询调用）
//   login_feishu_blocking()        -> LoginResult  阻塞式一步登录（后端开浏览器）
//
// 填充时机：M1（登录视图）时实现；出参类型见 ./types.ts。
// 约定：前端「两步非阻塞」流程 = start_login 拿码/链接 -> 展示 + opener 打开
//       verificationUrl -> 轮询 complete_login；也可视拍板结果改走阻塞式。
// ============================================================================

export {};
