<script setup lang="ts">
import { computed } from "vue";
import { generatePalette } from "../utils/colors";

const props = defineProps<{
  color: string | null;
  size?: number;
}>();

const size = props.size ?? 28;

// Default VS Code blue
const primaryColor = computed(() => props.color ?? "#32B5F1");
const palette = computed(() => generatePalette(primaryColor.value));
const stops = computed(() => palette.value.gradientStops);
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 32 32"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
  >
    <defs>
      <linearGradient
        id="vsTop"
        x1="23.79" y1="2" x2="23.79" y2="30"
        gradientUnits="userSpaceOnUse"
      >
        <stop :stop-color="stops[1]" />
        <stop offset="1" :stop-color="stops[2]" />
      </linearGradient>
      <linearGradient
        id="vsMid"
        x1="21" y1="5.53" x2="1.02" y2="22.3"
        gradientUnits="userSpaceOnUse"
      >
        <stop :stop-color="stops[4]" />
        <stop offset="0.27" :stop-color="stops[3]" />
        <stop offset="0.42" :stop-color="stops[3]" />
        <stop offset="0.62" :stop-color="stops[5]" />
        <stop offset="0.86" :stop-color="stops[4]" />
        <stop offset="1" :stop-color="stops[5]" />
      </linearGradient>
      <linearGradient
        id="vsBot"
        x1="1.16" y1="9.98" x2="21.08" y2="26.48"
        gradientUnits="userSpaceOnUse"
      >
        <stop :stop-color="stops[2]" />
        <stop offset="1" :stop-color="stops[3]" />
      </linearGradient>
    </defs>
    <path
      d="M21.0016 3.11679C21.0016 2.23783 20.0175 2.23782 19.5801 2.34769C20.1924 1.86426 20.9105 1.98147 21.1656 2.12796L27.079 5.02747C27.6424 5.30375 27.9998 5.8786 27.9998 6.50857V25.5831C27.9998 26.2215 27.6329 26.8025 27.058 27.0743L21.4937 29.7054C21.1109 29.8701 20.2799 30.2767 19.5801 29.7053C20.4549 29.8702 20.9287 29.2476 21.0016 28.8264V3.11679Z"
      fill="url(#vsTop)"
    />
    <path
      d="M19.6512 2.3319C20.1154 2.24017 21.0018 2.28271 21.0018 3.11685V9.68254L3.07359 23.2453C2.76022 23.4824 2.3192 23.443 2.05229 23.1542L0.204532 21.1548C-0.0849358 20.8416 -0.0646824 20.3513 0.249624 20.0633L19.5802 2.34775L19.6512 2.3319Z"
      fill="url(#vsMid)"
    />
    <path
      d="M21.0018 22.3708L3.07359 8.80801C2.76022 8.57094 2.3192 8.61028 2.05229 8.8991L0.204532 10.8985C-0.0849358 11.2117 -0.0646824 11.702 0.249624 11.9901L19.5802 29.7056C20.455 29.8704 20.9289 29.2478 21.0018 28.8266V22.3708Z"
      fill="url(#vsBot)"
    />
  </svg>
</template>
