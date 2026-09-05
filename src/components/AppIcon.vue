<script setup lang="ts">
// 应用图标集。CSP 限制 style-src 'self' 且 img-src 不含外部域，
// 因此图标全部内联为 SVG path，不引第三方图标库。
// 约定：24x24 视窗，stroke 描边风格，颜色继承 currentColor。

const props = withDefaults(
  defineProps<{
    name: string;
    size?: number;
    strokeWidth?: number;
  }>(),
  { size: 16, strokeWidth: 1.6 }
);

const CIRCLE = "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18";

const icons: Record<string, string[]> = {
  workbench: [
    "M3 3h8v8H3zM13 3h8v8h-8zM3 13h8v8H3zM13 13h8v8h-8z",
  ],
  history: [CIRCLE, "M12 7v5l3.5 2"],
  settings: [
    "M4 7h8M16 7h4M4 12h4M12 12h8M4 17h8M16 17h4",
    "M15 7a2 2 0 1 0 4 0 2 2 0 0 0-4 0M9 12a2 2 0 1 0 4 0 2 2 0 0 0-4 0M15 17a2 2 0 1 0 4 0 2 2 0 0 0-4 0",
  ],
  book: ["M5 4h11a2 2 0 0 1 2 2v14H7a2 2 0 0 1-2-2z", "M5 4v14", "M9 8h6"],
  check: ["M4 12.5l5 5L20 6.5"],
  "check-circle": [CIRCLE, "M8 12.5l2.5 2.5L16 9.5"],
  "close-circle": [CIRCLE, "M9 9l6 6M15 9l-6 6"],
  "alert-circle": [CIRCLE, "M12 8v5M12 16.5h.01"],
  "info-circle": [CIRCLE, "M12 11v5M12 8h.01"],
  chevronRight: ["M9 5l7 7-7 7"],
  chevronDown: ["M5 9l7 7 7-7"],
  folder: [
    "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  ],
  "folder-open": [
    "M3 8h18l-2.2 9a2 2 0 0 1-2 1.6H7.2a2 2 0 0 1-2-1.6z",
    "M3 8V7a2 2 0 0 1 2-2h3.4l2 2H19a2 2 0 0 1 2 2v1",
  ],
  doc: [
    "M7 3h7l5 5v11a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z",
    "M14 3v5h5",
    "M9 13h6M9 17h4",
  ],
  sheet: ["M3 5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z", "M3 10h18M9 10v10M15 10v10"],
  bitable: [
    "M3 5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
    "M3 9h18M3 14h18M9 9v10M15 9v10",
  ],
  paperclip: ["M20 11.5l-7.5 7.5a4.6 4.6 0 0 1-6.5-6.5l7.5-7.5a3 3 0 0 1 4.3 4.3l-7.6 7.6a1.5 1.5 0 0 1-2.1-2.1l6.8-6.8"],
  other: ["M4 6h16M4 12h16M4 18h10"],
  download: ["M12 4v11M7.5 10.5l4.5 4.5 4.5-4.5", "M5 19h14"],
  close: ["M6 6l12 12M18 6L6 18"],
  refresh: ["M20 12a8 8 0 1 1-2.6-5.9", "M20 4v5h-5"],
  link: [
    "M10 13a5 5 0 0 0 7 0l2.5-2.5a5 5 0 0 0-7-7L11.5 5",
    "M14 11a5 5 0 0 0-7 0L4.5 13.5a5 5 0 0 0 7 7L12.5 19",
  ],
  trash: ["M4 7h16M9 7V5h6v2M6 7l1 13h10l1-13"],
  spinner: ["M12 3a9 9 0 0 1 9 9"],
  external: ["M14 4h6v6", "M20 4l-9 9", "M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4"],
  plus: ["M12 5v14M5 12h14"],
  minus: ["M5 12h14"],
};
</script>

<template>
  <svg
    :width="props.size"
    :height="props.size"
    viewBox="0 0 24 24"
    fill="none"
    :stroke="'currentColor'"
    :stroke-width="props.strokeWidth"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    style="flex: none; display: block"
  >
    <path v-for="(d, i) in icons[props.name] || []" :key="i" :d="d" />
  </svg>
</template>
