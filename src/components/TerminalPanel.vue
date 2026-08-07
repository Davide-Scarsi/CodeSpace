<script lang="ts">
import { ref } from "vue";
// Persist across TerminalPanel mounts/unmounts
const exitedIds = ref<Set<string>>(new Set());
const persistedTabId = ref<string | null>(null);
</script>

<script setup lang="ts">
import { watch, onMounted, onUnmounted, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";
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
  closeWhenDone?: boolean;
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

  // Copy on Ctrl+C when text is selected
  term.attachCustomKeyEventHandler((e) => {
    if (e.ctrlKey && e.key === 'c' && term!.hasSelection()) {
      const sel = term!.getSelection();
      navigator.clipboard.writeText(sel).catch(() => {});
      return false; // prevent default (don't send to process)
    }
    if (e.ctrlKey && e.key === 'v') {
      navigator.clipboard.readText().then((text) => {
        if (activeTabId.value === id) {
          invoke("terminal_write", { terminalId: id, data: text }).catch(() => {});
        }
      }).catch(() => {});
      return false;
    }
    return true;
  });

  el.style.minHeight = "200px";
  setTimeout(() => { try { fitAddon.fit(); } catch (_) {} }, 100);
  const cached = outputCache.value[id] || "";
  console.log("[term] writing cached output:", cached.length, "chars");
  if (cached) term.write(colorizeOutput(cached, getActiveWsColor()));
});

let unlistenOut: (() => void) | null = null;
let unlistenExit: (() => void) | null = null;

function hexToRgb(hex: string): [number, number, number] | null {
  const m = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
  return m ? [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)] : null;
}

function ansiFg(hex: string): string {
  const rgb = hexToRgb(hex);
  return rgb ? `\x1b[38;2;${rgb[0]};${rgb[1]};${rgb[2]}m` : "";
}

const ANSI_RESET = "\x1b[0m";

function colorizeOutput(text: string, wsColor: string): string {
  const ws = ansiFg(wsColor || "#58a6ff");
  const g = ansiFg("#3fb950");
  const r = ansiFg("#f85149");
  const y = ansiFg("#d29922");
  const m = ansiFg("#bc8cff");
  const gray = ansiFg("#8b949e");
  const c = ansiFg("#39c5cf");

  let out = text;

  // Step markers: >>> ... (workspace color)
  out = out.replace(/^>>>.*$/gm, (s) => `${ws}${s}${ANSI_RESET}`);

  // Lines starting with OK or ending with OK (green)
  out = out.replace(/^OK\b.*$/gm, (s) => `${g}${s}${ANSI_RESET}`);

  // Separators (workspace color) — before other patterns to avoid conflicts
  out = out.replace(/={3,}/g, (s) => `${ws}${s}${ANSI_RESET}`);
  out = out.replace(/-{3,}/g, (s) => `${ws}${s}${ANSI_RESET}`);

  // URLs (workspace color)
  out = out.replace(/https?:\/\/[^\s\x1b]+/g, (s) => `${ws}${s}${ANSI_RESET}`);

  // IP:port (workspace color)
  out = out.replace(/(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})(:\d+)?/g, (m) => `${ws}${m}${ANSI_RESET}`);

  // [OK], [SUCCESS], ✓ (green)
  out = out.replace(/\[OK\]|\[SUCCESS\]|✓/gi, (s) => `${g}${s}${ANSI_RESET}`);

  // [ERR], [ERROR], [FAIL], ✗ (red)
  out = out.replace(/\[ERR\]|\[ERROR\]|\[FAIL\]|✗/gi, (s) => `${r}${s}${ANSI_RESET}`);

  // [WARN], [WARNING] (yellow)
  out = out.replace(/\[WARN\]|\[WARNING\]/gi, (s) => `${y}${s}${ANSI_RESET}`);

  // [UPLOAD], [DOWNLOAD], [SYNC] (magenta)
  out = out.replace(/\[UPLOAD\]|\[DOWNLOAD\]|\[SYNC\]/gi, (s) => `${m}${s}${ANSI_RESET}`);

  // [N/M] progression markers (gray)
  out = out.replace(/\[\d+\/\d+\]/g, (s) => `${gray}${s}${ANSI_RESET}`);

  return out;
}

function getActiveWsColor(): string {
  const tab = props.tabs.find(t => t.id === activeTabId.value);
  return tab?.color || props.activeColor || "#58a6ff";
}

onMounted(async () => {
  console.log("[TerminalPanel] mounted, tabs:", props.tabs.length);
  unlistenOut = await listen<{ terminalId: string; data: string }>("terminal-output", (e) => {
    const { terminalId, data } = e.payload;
    console.log("[term-out]", terminalId, data.substring(0, 60));
    // Store raw output, apply colors at display time
    outputCache.value[terminalId] = (outputCache.value[terminalId] || "") + data;
    if (term && activeTabId.value === terminalId) {
      term.write(colorizeOutput(data, getActiveWsColor()));
    }
  });

  unlistenExit = await listen<{ terminalId: string }>("terminal-exit", (e) => {
    console.log("[term-exit]", e.payload.terminalId);
    exitedIds.value.add(e.payload.terminalId);
    const tab = props.tabs.find(t => t.id === e.payload.terminalId);
    if (tab?.closeWhenDone) {
      // Auto-close tab after brief delay so user sees the exit message
      setTimeout(() => emit("close-tab", e.payload.terminalId), 1500);
    }
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
  padding: 2ex 2ch;
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
