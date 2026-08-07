<script lang="ts">
import { ref } from "vue";
// Persist across TerminalPanel mounts/unmounts
const exitedIds = ref<Set<string>>(new Set());
const persistedTabId = ref<string | null>(null);
</script>

<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, nextTick } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { listen } from "@tauri-apps/api/event";
import "@xterm/xterm/css/xterm.css";

export interface TerminalTab {
  id: string;
  label: string;
  taskType: string;
  color: string;
  url?: string;
}

const props = defineProps<{
  tabs: TerminalTab[];
  activeColor: string;
  taskIcon: string;
  taskType: string;
}>();

const emit = defineEmits<{
  "close-tab": [id: string];
  "select-tab": [id: string];
}>();

const activeTabId = ref<string | null>(null);
const terminalEl = ref<HTMLDivElement>();
let term: Terminal | null = null;
const fitAddon = new FitAddon();
const outputCache = ref<Record<string, string>>({});

let prevCount = 0;

watch(() => props.tabs, async (tabs) => {
  const count = tabs.length;
  console.log("[term] tabs changed:", count, "prev:", prevCount, "active:", activeTabId.value);
  if (count > 0) {
    if (count > prevCount) {
      // New tab added: always focus it
      const newTab = tabs[count - 1];
      console.log("[term] new tab detected, focusing:", newTab.id);
      activeTabId.value = newTab.id;
      persistedTabId.value = newTab.id;
    } else if (persistedTabId.value && tabs.some(t => t.id === persistedTabId.value)) {
      activeTabId.value = persistedTabId.value;
    } else if (!activeTabId.value || !tabs.some(t => t.id === activeTabId.value)) {
      activeTabId.value = tabs[0].id;
      persistedTabId.value = tabs[0].id;
    }
  } else {
    activeTabId.value = null;
    if (term) { term.dispose(); term = null; }
  }
  prevCount = count;
}, { immediate: true });

// Keep persisted tab in sync with active
watch(activeTabId, (id) => {
  if (id) persistedTabId.value = id;
});

watch(activeTabId, async (id) => {
  if (!id) return;
  // Wait for v-if div to render
  await nextTick();
  await nextTick();
  if (term) { term.dispose(); term = null; }
  const el = terminalEl.value;
  if (!el) { console.warn("[term] no terminalEl yet"); return; }
  console.log("[term] creating terminal for", id, "el size:", el.offsetWidth, el.offsetHeight);
  term = new Terminal({
    cursorBlink: true, fontSize: 13,
    fontFamily: "Consolas, 'Courier New', monospace",
    theme: {
      background: "#0d1117",
      foreground: "#c9d1d9",
      cursor: "#58a6ff",
      selectionBackground: "#1f6feb66",
      black: "#484f58",
      red: "#ff7b72",
      green: "#3fb950",
      yellow: "#d29922",
      blue: "#58a6ff",
      magenta: "#bc8cff",
      cyan: "#39c5cf",
      white: "#b1bac4",
      brightBlack: "#6e7681",
      brightRed: "#ffa198",
      brightGreen: "#56d364",
      brightYellow: "#e3b341",
      brightBlue: "#79c0ff",
      brightMagenta: "#d2a8ff",
      brightCyan: "#56d4dd",
      brightWhite: "#f0f6fc",
    },
  });
  term.loadAddon(fitAddon);
  term.open(el);
  el.style.minHeight = "200px";
  setTimeout(() => { try { fitAddon.fit(); } catch (_) {} }, 100);
  const cached = outputCache.value[id] || "";
  console.log("[term] writing cached output:", cached.length, "chars");
  if (cached) term.write(cached);
});

let unlistenOut: (() => void) | null = null;
let unlistenExit: (() => void) | null = null;

onMounted(async () => {
  console.log("[TerminalPanel] mounted, tabs:", props.tabs.length);
  unlistenOut = await listen<{ terminalId: string; data: string }>("terminal-output", (e) => {
    const { terminalId, data } = e.payload;
    console.log("[term-out]", terminalId, data.substring(0, 60));
    outputCache.value[terminalId] = (outputCache.value[terminalId] || "") + data;
    if (term && activeTabId.value === terminalId) {
      term.write(data);
    }
  });

  unlistenExit = await listen<{ terminalId: string }>("terminal-exit", (e) => {
    console.log("[term-exit]", e.payload.terminalId);
    exitedIds.value.add(e.payload.terminalId);
    if (term && activeTabId.value === e.payload.terminalId) {
      term.write("\r\n\n[Process exited]\r\n");
    }
  });

  // Handle resize
  const ro = new ResizeObserver(() => {
    if (fitAddon) {
      try { fitAddon.fit(); } catch (_) {}
    }
  });
  if (terminalEl.value) ro.observe(terminalEl.value);
});

onUnmounted(() => {
  if (unlistenOut) unlistenOut();
  if (unlistenExit) unlistenExit();
  if (term) term.dispose();
});
</script>

<template>
  <div class="terminal-panel" v-if="tabs.length > 0">
    <div class="term-tabs">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        class="term-tab"
        :class="{ active: tab.id === activeTabId }"
        :style="{ '--tab-color': tab.color || '#0078d4' }"
        @click="activeTabId = tab.id"
      >
        <img
          v-if="tab.taskType && tab.taskType !== 'default'"
          :src="`/icons/${tab.taskType}.svg`"
          width="16"
          height="16"
          class="term-tab-icon"
          :class="{ 'icon-spin': tab.taskType === 'sync' && !exitedIds.has(tab.id) }"
          alt=""
        />
        <span class="term-tab-label">{{ tab.label }}</span>
        <span class="term-tab-close" @click.stop="emit('close-tab', tab.id)">×</span>
      </button>
    </div>
    <div ref="terminalEl" class="term-body"></div>
  </div>
</template>

<style scoped>
.terminal-panel {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  background: #0d1117;
  border-left: 1px solid #3c3c3c;
}

.term-tabs {
  display: flex;
  background: #161b22;
  border-bottom: 1px solid #3c3c3c;
  overflow-x: auto;
  flex-shrink: 0;
  gap: 2px;
  padding: 4px 6px 0;
}

.term-tab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 10px;
  background: color-mix(in srgb, var(--tab-color, #0078d4) 12%, #0d1117);
  border: 1px solid #30363d;
  border-bottom: none;
  border-radius: 6px 6px 0 0;
  color: #fff;
  font-size: 12px;
  cursor: pointer;
  white-space: nowrap;
  flex-shrink: 0;
  transition: background 0.15s;
}

.term-tab:hover {
  background: color-mix(in srgb, var(--tab-color, #0078d4) 22%, #0d1117);
}

.term-tab.active {
  background: color-mix(in srgb, var(--tab-color, #0078d4) 25%, #0d1117);
  border-color: var(--tab-color, #3c3c3c);
}

.term-tab-icon {
  flex-shrink: 0;
  opacity: 0.85;
}

.icon-spin {
  animation: tab-icon-spin 3s linear infinite;
}

@keyframes tab-icon-spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.term-tab-label {
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.term-tab-close {
  font-size: 15px;
  line-height: 1;
  opacity: 0.5;
  border-radius: 4px;
  padding: 0 4px;
  transition: opacity 0.15s, background 0.15s;
  margin-left: 2px;
}

.term-tab-close:hover {
  opacity: 1;
  background: rgba(248, 81, 73, 0.35);
  color: #f85149;
}

.term-body {
  flex: 1;
  min-height: 0;
  padding: 0;
}

.term-body :deep(.xterm) {
  height: 100%;
}

.term-body :deep(.xterm-viewport) {
  scrollbar-width: thin;
  scrollbar-color: #30363d #0d1117;
}

.term-body :deep(.xterm-viewport::-webkit-scrollbar) {
  width: 8px;
}

.term-body :deep(.xterm-viewport::-webkit-scrollbar-track) {
  background: #0d1117;
}

.term-body :deep(.xterm-viewport::-webkit-scrollbar-thumb) {
  background: #30363d;
  border-radius: 4px;
}

.term-body :deep(.xterm-viewport::-webkit-scrollbar-thumb:hover) {
  background: #484f58;
}
</style>
