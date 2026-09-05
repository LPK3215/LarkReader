<!--
  DirPicker —— 输出目录选择控件（结构占位）
  填充时机：M1（默认目录引导）先可用、M2（工作台）复用。
  职责：@tauri-apps/plugin-dialog 的 open({ directory: true }) 选目录 ->
  api/settings.ts preflight_output_dir 校验可写/可用空间 -> 展示 OutputPreflight。
  注：dialog 插件需后端执行 `pnpm tauri add dialog` 并补 capabilities 权限后再接。
-->
<script setup lang="ts">
// TODO(M1): dialog 选目录 + preflight 展示。
</script>

<template>
  <div />
</template>
