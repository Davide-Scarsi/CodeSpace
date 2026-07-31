<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import VscodeIcon from "./components/VscodeIcon.vue";

// ── Types ─────────────────────────────────────────────────
interface WorkspaceInfo {
  path: string;
  name: string;
  display_path: string;
  color: string | null;
}

// ── State ────────────────────────────────────────────────
const workspaces = ref<WorkspaceInfo[]>([]);
const scanInfo = ref({ has_cache: false, count: 0, last_scan: 0 });
const scanning = ref(false);
const searchQuery = ref("");
const statusMessage = ref("");

// Context menu
const contextMenu = ref<{ x: number; y: number; ws: WorkspaceInfo } | null>(null);

// ── Computed ─────────────────────────────────────────────
const filteredWorkspaces = computed(() => {
  if (!searchQuery.value.trim()) return workspaces.value;
  const q = searchQuery.value.toLowerCase();
  return workspaces.value.filter(
    (ws) =>
      ws.name.toLowerCase().includes(q) ||
      ws.display_path.toLowerCase().includes(q)
  );
});

const lastScanDate = computed(() => {
  if (!scanInfo.value.last_scan) return "";
  return new Date(scanInfo.value.last_scan * 1000).toLocaleString();
});

// ── Actions ──────────────────────────────────────────────
async function loadScanInfo() {
  scanInfo.value = await invoke("get_scan_info");
}

async function scan(forceFull: boolean) {
  scanning.value = true;
  statusMessage.value = forceFull
    ? "Full scan in progress (scanning all drives)..."
    : "Scanning...";
  try {
    workspaces.value = await invoke("scan_workspaces", { forceFull });
    await loadScanInfo();
    statusMessage.value = `Found ${scanInfo.value.count} workspace(s)`;
  } catch (e) {
    statusMessage.value = `Error: ${e}`;
  } finally {
    scanning.value = false;
  }
}

async function launchWorkspace(path: string) {
  try {
    await invoke("launch_workspace", { path });
    statusMessage.value = `Launched: ${path}`;
  } catch (e) {
    statusMessage.value = `Launch error: ${e}`;
  }
}

// ── Context Menu ─────────────────────────────────────────
const PEAKOCK_COLORS = [
  "#007fff", "#ff007f", "#00bcd4", "#00ff7f", "#9c27b0",
  "#ff5722", "#ffc107", "#3f51b5", "#8bc34a", "#e91e63",
  "#009688", "#607d8b", "#1857a4", "#dd0531", "#832561",
];

function onContextMenu(e: MouseEvent, ws: WorkspaceInfo) {
  e.preventDefault();
  contextMenu.value = { x: e.clientX, y: e.clientY, ws };
}

function closeContextMenu() {
  contextMenu.value = null;
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") closeContextMenu();
}

async function pickColor(ws: WorkspaceInfo, color: string) {
  try {
    await invoke("set_workspace_color", { workspacePath: ws.path, color });
    ws.color = color;
    statusMessage.value = `Color set to ${color} for ${ws.name}`;
  } catch (e) {
    statusMessage.value = `Error setting color: ${e}`;
  }
  closeContextMenu();
}

// ── Lifecycle ────────────────────────────────────────────
onMounted(async () => {
  document.addEventListener("keydown", onKeydown);
  await loadScanInfo();
  if (scanInfo.value.has_cache) {
    await scan(false);
  }
});

onUnmounted(() => {
  document.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <div class="app-shell">
    <!-- Header -->
    <header class="app-header">
      <h1 class="app-title">CodeSpace</h1>
      <span class="app-subtitle">VS Code Workspace Manager</span>
    </header>

    <!-- Toolbar -->
    <div class="toolbar">
      <button
        class="btn btn-primary"
        :disabled="scanning"
        @click="scan(false)"
      >
        {{ scanning ? "⏳ Scanning..." : "🔄 Quick Scan" }}
      </button>
      <button
        class="btn btn-secondary"
        :disabled="scanning"
        @click="scan(true)"
      >
        🔍 Full Scan
      </button>

      <div class="search-box">
        <span class="search-icon">🔎</span>
        <input
          v-model="searchQuery"
          type="text"
          placeholder="Filter workspaces..."
          class="search-input"
        />
        <span v-if="searchQuery" class="search-clear" @click="searchQuery = ''">✕</span>
      </div>

      <div class="toolbar-spacer"></div>

      <span v-if="scanInfo.has_cache" class="cache-badge">
        {{ scanInfo.count }} ws · {{ lastScanDate }}
      </span>
    </div>

    <!-- Status bar -->
    <div v-if="statusMessage" class="status-bar">{{ statusMessage }}</div>

    <!-- Workspace List -->
    <div class="list-container">
      <div v-if="workspaces.length === 0 && !scanning" class="empty-state">
        <p>No workspaces found.</p>
        <p class="hint">Click "Quick Scan" or "Full Scan" to start.</p>
      </div>

      <div
        v-for="ws in filteredWorkspaces"
        :key="ws.path"
        class="ws-card"
        @click="launchWorkspace(ws.path)"
        @contextmenu="onContextMenu($event, ws)"
      >
        <div class="ws-icon">
          <VscodeIcon :color="ws.color" :size="28" />
        </div>
        <div class="ws-info">
          <span class="ws-name">{{ ws.name }}</span>
          <span class="ws-path">{{ ws.display_path }}</span>
        </div>
      </div>
    </div>

    <!-- Context Menu -->
    <Teleport to="body">
      <div
        v-if="contextMenu"
        class="context-menu-overlay"
        @click="closeContextMenu"
        @contextmenu.prevent="closeContextMenu"
      >
        <div
          class="context-menu"
          :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
          @click.stop
        >
          <div class="context-menu-title">🎨 Peacock color</div>
          <div class="color-grid">
            <button
              v-for="c in PEAKOCK_COLORS"
              :key="c"
              class="color-swatch"
              :style="{ background: c }"
              :class="{ active: contextMenu.ws.color === c }"
              :title="c"
              @click="pickColor(contextMenu.ws, c)"
            ></button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style>
/* ── Reset & Base ──────────────────────────────────────── */
*,
*::before,
*::after {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

:root {
  font-family: "Segoe UI", Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 14px;
  color: #c9d1d9;
  background: #0d1117;
}

body {
  overflow: hidden;
}
</style>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #0d1117;
}

/* ── Header ───────────────────────────────────────────── */
.app-header {
  display: flex;
  align-items: baseline;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid #21262d;
  background: #161b22;
  flex-shrink: 0;
}

.app-title {
  font-size: 18px;
  font-weight: 600;
  color: #58a6ff;
}

.app-subtitle {
  font-size: 12px;
  color: #8b949e;
}

/* ── Toolbar ──────────────────────────────────────────── */
.toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  border-bottom: 1px solid #21262d;
  background: #161b22;
  flex-shrink: 0;
}

.toolbar-spacer {
  flex: 1;
}

.btn {
  padding: 6px 14px;
  border: 1px solid #30363d;
  border-radius: 6px;
  font-size: 13px;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
  white-space: nowrap;
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-primary {
  background: #238636;
  color: #fff;
  border-color: #2ea043;
}

.btn-primary:hover:not(:disabled) {
  background: #2ea043;
}

.btn-secondary {
  background: #21262d;
  color: #c9d1d9;
}

.btn-secondary:hover:not(:disabled) {
  background: #30363d;
}

/* ── Search ───────────────────────────────────────────── */
.search-box {
  display: flex;
  align-items: center;
  gap: 6px;
  background: #0d1117;
  border: 1px solid #30363d;
  border-radius: 6px;
  padding: 4px 10px;
  flex: 1;
  max-width: 320px;
}

.search-icon {
  font-size: 14px;
  flex-shrink: 0;
}

.search-input {
  flex: 1;
  background: transparent;
  border: none;
  color: #c9d1d9;
  font-size: 13px;
  outline: none;
}

.search-input::placeholder {
  color: #484f58;
}

.search-clear {
  cursor: pointer;
  color: #8b949e;
  font-size: 14px;
  padding: 0 2px;
}

.search-clear:hover {
  color: #c9d1d9;
}

/* ── Cache badge ──────────────────────────────────────── */
.cache-badge {
  font-size: 11px;
  color: #8b949e;
  white-space: nowrap;
}

/* ── Status bar ───────────────────────────────────────── */
.status-bar {
  padding: 6px 16px;
  font-size: 12px;
  color: #8b949e;
  background: #161b22;
  border-bottom: 1px solid #21262d;
  flex-shrink: 0;
}

/* ── List container ───────────────────────────────────── */
.list-container {
  flex: 1;
  overflow: auto;
  padding: 4px 8px;
  scrollbar-width: thin;
  scrollbar-color: #30363d transparent;
}

.list-container::-webkit-scrollbar {
  width: 6px;
}
.list-container::-webkit-scrollbar-track {
  background: transparent;
}
.list-container::-webkit-scrollbar-thumb {
  background: #30363d;
  border-radius: 3px;
}
.list-container::-webkit-scrollbar-thumb:hover {
  background: #484f58;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: #8b949e;
  gap: 8px;
}

.empty-state .hint {
  font-size: 13px;
  color: #484f58;
}

/* ── Workspace card ───────────────────────────────────── */
.ws-card {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  margin: 2px 0;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.12s;
  border: 1px solid transparent;
}

.ws-card:hover {
  background: #1c2128;
  border-color: #30363d;
}

.ws-card:active {
  background: #1a2332;
}

.ws-icon {
  flex-shrink: 0;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.ws-info {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
}

.ws-name {
  font-size: 13px;
  font-weight: 600;
  color: #e6edf3;
}

.ws-path {
  font-size: 11px;
  color: #8b949e;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin-top: 1px;
}

/* ── Context menu ─────────────────────────────────────── */
.context-menu-overlay {
  position: fixed;
  inset: 0;
  z-index: 9998;
}

.context-menu {
  position: fixed;
  z-index: 9999;
  background: #1c2128;
  border: 1px solid #30363d;
  border-radius: 8px;
  padding: 8px 10px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
}

.context-menu-title {
  font-size: 12px;
  color: #8b949e;
  margin-bottom: 6px;
  padding: 0 2px;
}

.color-grid {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 6px;
}

.color-swatch {
  width: 28px;
  height: 28px;
  border-radius: 4px;
  border: 2px solid transparent;
  cursor: pointer;
  transition: border-color 0.1s, transform 0.1s;
}

.color-swatch:hover {
  border-color: #8b949e;
  transform: scale(1.15);
}

.color-swatch.active {
  border-color: #fff;
  box-shadow: 0 0 6px rgba(255, 255, 255, 0.3);
}
</style>