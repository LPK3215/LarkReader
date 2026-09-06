// ============================================================================
// src/composables/useMessage.ts —— 全局消息/对话框
//
// naive-ui 的 useMessage/useDialog 必须在 setup() 调用；本文件把 App.vue 里
// 创建的 message/dialog 实例提到模块层，让 store 内或组件外的事件回调也能弹。
//
// 注意：naive-ui 的 provider 是「嵌套挂载」关系——必须先在 App.vue 模板里挂
// NMessageProvider / NDialogProvider，调用 useMessage() 才返回真实实例，否则
// 会抛 "No outer <n-message-provider />"。本模块只暴露 holder/createApi 工具，
// 实例化在 App.vue 里完成。
// ============================================================================

import { createDiscreteApi } from "naive-ui";

// 离散 API（不依赖组件树）。可在 store / composable 里直接调用。
// 类型只取我们用到的，避免全局 typecheck 把所有 naive-ui 组件都拉进来。
const discrete = createDiscreteApi(["message", "dialog"]);

export const message = {
  info(content: string) {
    discrete.message.info(content);
  },
  success(content: string) {
    discrete.message.success(content);
  },
  warning(content: string) {
    discrete.message.warning(content);
  },
  error(content: string) {
    discrete.message.error(content);
  },
};

export const dialog = {
  warning(opts: {
    title: string;
    content: string;
    positiveText?: string;
    negativeText?: string;
    onPositiveClick?: () => void;
    onNegativeClick?: () => void;
  }) {
    discrete.dialog.warning({
      title: opts.title,
      content: opts.content,
      positiveText: opts.positiveText ?? "确定",
      negativeText: opts.negativeText ?? "取消",
      onPositiveClick: opts.onPositiveClick,
      onNegativeClick: opts.onNegativeClick,
    });
  },
  info(opts: {
    title: string;
    content: string;
    positiveText?: string;
    onPositiveClick?: () => void;
  }) {
    discrete.dialog.info({
      title: opts.title,
      content: opts.content,
      positiveText: opts.positiveText ?? "确定",
      onPositiveClick: opts.onPositiveClick,
    });
  },
};

/**
 * 把 invoke 抛出的错误统一转成可读文本。
 * Tauri 后端 Err(AppError) 到前端是一个对象（{code, message, ...}），
 * 直接 String() 会得到 "[object Object]"，这里优先取 message 字段。
 */
export function errMsg(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "object" && err !== null) {
    const message = (err as { message?: unknown }).message;
    if (typeof message === "string" && message) return message;
    try {
      return JSON.stringify(err);
    } catch {
      return String(err);
    }
  }
  return String(err);
}