// ============================================================================
// src/api/env.ts —— 环境体检 / 初始化命令封装（结构占位）
//
// 对应后端命令（src-tauri/src/commands.rs）：
//   check_env()            -> EnvStatus（node / lark_cli / app_configured / 登录态）
//   setup_lark_cli()       -> Result<String, AppError>   自动安装 lark-cli
//   init_app(brand, lang)  -> Result<String, AppError>   初始化飞书应用（阻塞，开浏览器）
//
// 填充时机：M1（引导页）时实现；出参类型见 ./types.ts。
// 约定：invoke 入参统一 camelCase（initApp / ...），错误统一走 normalizeError。
// ============================================================================

export {};
