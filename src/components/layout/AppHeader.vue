<script setup lang="ts">
// 顶部应用栏（44px）：品牌区 + 环境状态胶囊 + 设置入口。
// 环境状态由 App.vue 从 stores/auth.ts 的 EnvStatus 推导后经 props 传入。
// 状态胶囊可点击，进入「飞书终端」页做手动体检/登录/退出。

import AppIcon from "../AppIcon.vue";

type EnvLevel = "ready" | "warning" | "error";

withDefaults(
  defineProps<{
    level?: EnvLevel;
    text?: string;
    userName?: string | null;
  }>(),
  { level: "ready", text: "环境正常", userName: null }
);

const emit = defineEmits<{ openSettings: []; openEnv: [] }>();
</script>

<template>
  <header class="lr-header">
    <div class="lr-header__brand">
      <span class="lr-header__logo">
        <AppIcon name="doc" :size="14" :stroke-width="1.8" />
      </span>
      <span class="lr-header__name">LarkReader</span>
    </div>

    <div class="lr-header__right">
      <button
        class="lr-env"
        :class="`lr-env--${level}`"
        title="查看飞书终端状态并手动管理"
        @click="emit('openEnv')"
      >
        <i class="lr-env__dot" />
        {{ text }}
      </button>

      <span v-if="userName" class="lr-header__user lr-selectable">{{ userName }}</span>

      <button class="lr-iconbtn" title="设置" @click="emit('openSettings')">
        <AppIcon name="settings" :size="16" />
      </button>
    </div>
  </header>
</template>

<style scoped>
.lr-header {
  height: var(--lr-header-h);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 var(--lr-space-4);
  background: var(--lr-bg-surface);
  border-bottom: 0.5px solid var(--lr-border);
}

.lr-header__brand {
  display: flex;
  align-items: center;
  gap: var(--lr-space-2);
}

.lr-header__logo {
  width: 22px;
  height: 22px;
  border-radius: var(--lr-radius-md);
  background: var(--lr-primary);
  color: var(--lr-on-primary);
  display: flex;
  align-items: center;
  justify-content: center;
}

.lr-header__name {
  font-size: var(--lr-fs-section);
  font-weight: var(--lr-fw-medium);
  letter-spacing: 0.2px;
}

.lr-header__right {
  display: flex;
  align-items: center;
  gap: var(--lr-space-3);
}

.lr-env {
  display: inline-flex;
  align-items: center;
  gap: var(--lr-space-2);
  height: 22px;
  padding: 0 10px;
  border-radius: 11px;
  font-family: inherit;
  font-size: var(--lr-fs-secondary);
  background: var(--lr-success-soft);
  color: var(--lr-success);
  border: 0.5px solid var(--lr-success-border);
  cursor: pointer;
  transition: filter 0.15s, opacity 0.15s;
}

.lr-env:hover {
  filter: brightness(0.96);
}

.lr-env:active {
  opacity: 0.85;
}

.lr-env--warning {
  background: var(--lr-warning-soft);
  color: var(--lr-warning);
  border-color: var(--lr-warning-border);
}

.lr-env--error {
  background: var(--lr-danger-soft);
  color: var(--lr-danger);
  border-color: var(--lr-danger-border);
}

.lr-env__dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
}

.lr-header__user {
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-secondary);
}

.lr-iconbtn {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: var(--lr-radius-md);
  background: transparent;
  color: var(--lr-text-secondary);
  transition: background 0.15s, color 0.15s;
}

.lr-iconbtn:hover {
  background: var(--lr-bg-hover);
  color: var(--lr-text);
}
</style>
