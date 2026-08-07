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

watch(() => props.tabs, async (tabs) => {
  console.log("[term] tabs changed:", tabs.length);
  if (tabs.length > 0 && !activeTabId.value) {
    activeTabId.value = tabs[0].id;
  }
  if (tabs.length === 0) {
    activeTabId.value = null;
    if (term) { term.dispose(); term = null; }
  }
}, { immediate: true, deep: true });

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
    theme: { background: "#1e1e1e", foreground: "#d4d4d4", cursor: "#ffffff", selectionBackground: "#264f78" },
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
  background: #1e1e1e;
}

.term-tabs {
  display: flex;
  background: #252526;
  border-bottom: 1px solid #3c3c3c;
  overflow-x: auto;
  flex-shrink: 0;
}

.term-tab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: #2d2d2d;
  border: none;
  border-right: 1px solid #3c3c3c;
  color: #969696;
  font-size: 12px;
  cursor: pointer;
  white-space: nowrap;
  flex-shrink: 0;
  transition: background 0.15s;
}

.term-tab:hover {
  background: #3c3c3c;
  color: #ccc;
}

.term-tab.active {
  background: #1e1e1e;
  color: #fff;
  border-top: 2px solid var(--tab-color);
  padding-top: 4px;
}

.term-tab-label {
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.term-tab-close {
  font-size: 14px;
  line-height: 1;
  opacity: 0.5;
  border-radius: 3px;
  padding: 0 3px;
}

.term-tab-close:hover {
  opacity: 1;
  background: rgba(255,255,255,0.1);
}

.term-body {
  flex: 1;
  min-height: 200px;
  padding: 4px;
}
</style>
