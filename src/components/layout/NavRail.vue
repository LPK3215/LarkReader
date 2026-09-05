<script setup lang="ts">
// 左侧图标导航栏（52px）：工作台 / 历史 / 设置 / 本地阅读（M3 未开放，置灰）。
// 用图标栏而非带文字侧边栏：1024~1180 窗口下，省下的 128px 全部让给节点树。

import AppIcon from "../AppIcon.vue";

export interface NavItem {
  key: string;
  label: string;
  icon: string;
  route: string;
  disabled?: boolean;
}

const items: NavItem[] = [
  { key: "workspace", label: "工作台", icon: "workbench", route: "/workspace" },
  { key: "history", label: "任务历史", icon: "history", route: "/history" },
  { key: "reader", label: "本地阅读（规划中）", icon: "book", route: "/reader", disabled: true },
  { key: "settings", label: "设置", icon: "settings", route: "/settings" },
];
</script>

<script lang="ts">
export default { name: "NavRail" };
</script>

<template>
  <nav class="lr-nav">
    <RouterLink
      v-for="item in items"
      :key="item.key"
      :to="item.route"
      class="lr-nav__item"
      :class="{ 'is-active': $route.path === item.route, 'is-disabled': item.disabled }"
      :title="item.label"
      :tabindex="item.disabled ? -1 : 0"
      @click="item.disabled && $event.preventDefault()"
    >
      <AppIcon :name="item.icon" :size="18" />
    </RouterLink>
  </nav>
</template>

<style scoped>
.lr-nav {
  width: var(--lr-nav-w);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--lr-space-2);
  padding: var(--lr-space-3) 0;
  background: var(--lr-bg-surface);
  border-right: 0.5px solid var(--lr-border);
}

.lr-nav__item {
  width: 34px;
  height: 34px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--lr-radius-md);
  color: var(--lr-text-secondary);
  text-decoration: none;
  transition: background 0.15s, color 0.15s;
}

.lr-nav__item:hover {
  background: var(--lr-bg-hover);
  color: var(--lr-text);
}

.lr-nav__item.is-active {
  background: var(--lr-primary-soft);
  color: var(--lr-primary);
}

.lr-nav__item.is-disabled {
  color: var(--lr-text-disabled);
  cursor: not-allowed;
  pointer-events: none;
}
</style>
