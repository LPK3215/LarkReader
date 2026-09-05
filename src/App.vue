<script setup lang="ts">
// ============================================================================
// App.vue —— 应用外壳
//
// 布局：顶栏(44) + [图标导航(52) | 主区] + 全局任务条(40，任务运行时出现)
// Onboarding 走 meta.bare，全屏无壳，完成即消失。
// 全局任务条跨页面保留：切到历史或设置页也能看到进度并取消。
// ============================================================================

import { computed, watch } from "vue";
import { useRoute, useRouter } from "vue-router";

import AppHeader from "./components/layout/AppHeader.vue";
import NavRail from "./components/layout/NavRail.vue";
import GlobalTaskBar from "./components/layout/GlobalTaskBar.vue";
import { useTaskStore } from "./stores/task";
import { useSettingsStore } from "./stores/settings";
import { message as globalMessage } from "./composables/useMessage";

const route = useRoute();
const router = useRouter();
const task = useTaskStore();
const settings = useSettingsStore();

/** Onboarding 全屏无壳 */
const bare = computed(() => route.meta.bare === true);

const showTaskBar = computed(
  () => !bare.value && task.taskBarVisible && (task.running || task.finished)
);

/** 全局错误：task / settings store 抛错时统一弹 toast。 */
watch(
  () => task.lastError,
  (msg) => {
    if (msg) globalMessage.error(msg);
  }
);
watch(
  () => settings.warning,
  (msg) => {
    if (msg) globalMessage.warning(msg);
  }
);

function goSettings() {
  router.push("/settings");
}

function goWorkspace() {
  router.push("/workspace");
}
</script>

<template>
  <!-- 引导页：全屏，不套外壳 -->
  <RouterView v-if="bare" />

  <!-- 正常外壳 -->
  <div v-else class="lr-shell">
    <AppHeader level="ready" text="环境正常" @open-settings="goSettings" />

    <div class="lr-shell__body">
      <NavRail />
      <main class="lr-shell__main">
        <RouterView />
      </main>
    </div>

    <GlobalTaskBar
      v-if="showTaskBar"
      :phase-label="task.phaseLabel"
      :done="task.done"
      :total="task.total"
      :current-doc="task.currentDoc"
      :remaining="
        task.estimatedRemainingSeconds ? `${task.estimatedRemainingSeconds} 秒` : null
      "
      :success-count="task.successCount"
      :failed-count="task.failedCount"
      :cancelled="task.cancelled"
      @cancel="task.cancel()"
      @minimize="task.taskBarVisible = false"
      @detail="goWorkspace"
    />
  </div>
</template>

<style scoped>
.lr-shell {
  height: 100%;
  display: grid;
  grid-template-rows: var(--lr-header-h) minmax(0, 1fr) auto;
  background: var(--lr-bg-app);
}

.lr-shell__body {
  display: grid;
  grid-template-columns: var(--lr-nav-w) minmax(0, 1fr);
  min-height: 0;
}

.lr-shell__main {
  min-height: 0;
  overflow: hidden;
  padding: var(--lr-space-5);
}
</style>
