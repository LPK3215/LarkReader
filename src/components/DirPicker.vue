<script setup lang="ts">
// ============================================================================
// DirPicker —— 输出目录选择控件
//
// 流程：dialog.open({ directory: true }) 选目录 -> preflight_output_dir 校验
//      可写性与磁盘空间 -> 展示 OutputPreflight（不可写时给出错误态）
//
// 环境降级：在纯浏览器（pnpm dev 且非 Tauri 窗口）里 dialog 不可用，
//          捕获异常后保留手动输入，不阻塞开发与视觉验证。
// ============================================================================

import { ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import AppIcon from "./AppIcon.vue";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    availableText?: string | null;
    writable?: boolean;
    error?: string | null;
    compact?: boolean;
  }>(),
  { availableText: null, writable: true, error: null, compact: false }
);

const emit = defineEmits<{ "update:modelValue": [string] }>();

const picking = ref(false);

async function pick() {
  picking.value = true;
  try {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string" && selected) {
      emit("update:modelValue", selected);
    }
  } catch (err) {
    // dialog 失败（用户取消、权限问题）静默忽略，保留手动输入
  } finally {
    picking.value = false;
  }
}
</script>

<template>
  <div class="lr-dirpicker">
    <div class="lr-dirpicker__row">
      <input
        :value="props.modelValue"
        class="lr-input lr-input--mono lr-selectable"
        :class="{ 'lr-input--error': !!props.error }"
        placeholder="选择导出到的目录"
        @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
      />
      <button class="lr-btn lr-btn--secondary" :disabled="picking" @click="pick">
        <AppIcon v-if="!picking" name="folder-open" :size="14" />
        {{ picking ? "选择中…" : "浏览" }}
      </button>
    </div>

    <p v-if="props.error" class="lr-dirpicker__msg lr-dirpicker__msg--bad">
      <AppIcon name="alert-circle" :size="12" />
      {{ props.error }}
    </p>
    <p v-else-if="props.availableText" class="lr-dirpicker__msg">
      <AppIcon name="info-circle" :size="12" />
      可用空间 {{ props.availableText }}
    </p>
  </div>
</template>

<style scoped>
.lr-dirpicker {
  display: flex;
  flex-direction: column;
  gap: var(--lr-space-2);
}

.lr-dirpicker__row {
  display: flex;
  gap: var(--lr-space-2);
}

.lr-input--error {
  border-color: var(--lr-danger);
}

.lr-dirpicker__msg {
  display: flex;
  align-items: center;
  gap: var(--lr-space-1);
  font-size: var(--lr-fs-secondary);
  color: var(--lr-text-tertiary);
}

.lr-dirpicker__msg--bad {
  color: var(--lr-danger);
}
</style>
