<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import VscodeIcon from "./components/VscodeIcon.vue";
import UpdateBanner from "./components/UpdateBanner.vue";

declare const __APP_VERSION__: string;
const appVersion = __APP_VERSION__;

// ── Types ─────────────────────────────────────────────────
interface WorkspaceInfo {
  path: string;
  name: string;
  display_path: string;
  color: string | null;
  is_open: boolean;
}

// ── State ────────────────────────────────────────────────
const workspaces = ref<WorkspaceInfo[]>([]);
const scanInfo = ref({ has_cache: false, count: 0, last_scan: 0 });
const scanning = ref(false);
const searchQuery = ref("");
const statusMessage = ref("");
const dragOver = ref(false);

// Tauri native drag-drop
let unlisten: (() => void) | null = null;

async function setupDragDrop() {
  unlisten = await getCurrentWindow().onDragDropEvent(async (event) => {
    if (event.payload.type === "over") {
      dragOver.value = true;
    } else if (event.payload.type === "leave") {
      dragOver.value = false;
    } else if (event.payload.type === "drop") {
      dragOver.value = false;
      const paths = event.payload.paths;
      let created = 0;
      for (const folderPath of paths) {
        try {
          const wsPath = await invoke("create_workspace", { folderPath });
          statusMessage.value = `Created: ${wsPath}`;
          created++;
        } catch (_) {
          // Not a folder or error — skip
        }
      }
      if (created > 0) {
        await scan(false);
      }
    }
  });
}

// Color Modal
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

async function focusWorkspace(name: string) {
  try {
    await invoke("focus_workspace", { name });
    // Force update: backend confirmed focus, skip waiting for monitor poll
    debugActiveName.value = name;
  } catch (e) {
    statusMessage.value = `Focus error: ${e}`;
  }
}

// ── Color Modal ─────────────────────────────────────────
const modalWs = ref<WorkspaceInfo | null>(null);

const PEAKOCK_COLORS = [
  "#007fff", "#ff007f", "#00bcd4", "#00ff7f", "#9c27b0",
  "#ff5722", "#ffc107", "#3f51b5", "#8bc34a", "#e91e63",
  "#009688", "#607d8b", "#1857a4", "#dd0531", "#832561",
];

function openColorModal(ws: WorkspaceInfo) {
  modalWs.value = ws;
}

function closeColorModal() {
  modalWs.value = null;
}

async function pickColor(color: string) {
  const ws = modalWs.value;
  if (!ws) return;
  try {
    // If clicking the already-active color, remove it (reset to default)
    if (ws.color?.toLowerCase() === color.toLowerCase()) {
      await invoke("remove_workspace_color", { workspacePath: ws.path });
      ws.color = null;
      statusMessage.value = `Color removed for ${ws.name}`;
    } else {
      await invoke("set_workspace_color", { workspacePath: ws.path, color });
      ws.color = color;
      statusMessage.value = `Color set to ${color} for ${ws.name}`;
    }
  } catch (e) {
    statusMessage.value = `Error: ${e}`;
  }
  closeColorModal();
}

// ── Lifecycle ────────────────────────────────────────────
const activeWorkspace = computed(() => {
  if (debugActiveName.value) {
    return workspaces.value.find(w => w.name === debugActiveName.value) || null;
  }
  // Fallback: first open workspace
  return workspaces.value.find(w => w.is_open) || null;
});

let unlistenWs: (() => void) | null = null;
const debugOpenNames = ref<string[]>([]);
const debugActiveName = ref<string | null>(null);

onMounted(async () => {
  setupDragDrop();
  await loadScanInfo();
  if (scanInfo.value.has_cache) {
    await scan(false);
  }

  // Start background monitor (Win32 API, event-driven)
  // console.log("[DEBUG] calling start_workspace_monitor");
  await invoke("start_workspace_monitor");
  // console.log("[DEBUG] start_workspace_monitor returned, setting up listener");
  unlistenWs = await listen<{ open_names: string[]; active_name: string | null }>("workspace-changed", (event) => {
    const { open_names, active_name } = event.payload;
    // console.log("[DEBUG] workspace-changed received:", JSON.stringify(event.payload));
    debugOpenNames.value = open_names;
    // Keep last known active workspace when none has focus
    if (active_name !== null) {
      debugActiveName.value = active_name;
    } else if (debugActiveName.value && !open_names.includes(debugActiveName.value)) {
      // Last active workspace was closed — clear it so fallback picks another open one
      debugActiveName.value = null;
    }
    workspaces.value.forEach((ws) => {
      ws.is_open = open_names.includes(ws.name);
    });
  });
  // console.log("[DEBUG] listener set up");
});

onUnmounted(() => {
  if (unlisten) unlisten();
  if (unlistenWs) unlistenWs();
  invoke("stop_workspace_monitor").catch(() => {});
});
</script>

<template>
  <div class="app-shell">
    <!-- Header -->
    <header class="app-header">
      <h1 class="app-title">CodeSpace</h1>
      <span class="app-subtitle">VS Code Workspace Manager</span>
    </header>

    <UpdateBanner />

    <!-- DEBUG banner rimosso -->
    <!-- Toolbar -->
    <div class="toolbar">
      <button
        class="btn btn-primary"
        :disabled="scanning"
        @click="scan(false)"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><path d="M21 12a9 9 0 1 1-6.8-8.7"/><path d="M21 3v6h-6"/></svg>
        {{ scanning ? "Scanning..." : "Quick Scan" }}
      </button>
      <button
        class="btn btn-secondary"
        :disabled="scanning"
        @click="scan(true)"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="6"/><circle cx="12" cy="12" r="2"/><path d="M12 2v4"/></svg>
        Full Scan
      </button>

      <div class="search-box">
        <svg class="search-icon-svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>
        <input
          v-model="searchQuery"
          type="text"
          placeholder="Filter workspaces..."
          class="search-input"
        />
        <span v-if="searchQuery" class="search-clear" @click="searchQuery = ''">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>
        </span>
      </div>

      <div class="toolbar-spacer"></div>

      <span v-if="scanInfo.has_cache" class="cache-badge">
        {{ scanInfo.count }} ws · {{ lastScanDate }}
      </span>
    </div>

    <!-- Status bar -->
    <div v-if="statusMessage" class="status-bar">{{ statusMessage }}</div>

    <!-- Active workspace banner -->
    <div v-if="activeWorkspace" class="active-banner" :style="{ '--ws-color': activeWorkspace.color || '#0078d4' }">
      <span class="active-name">{{ activeWorkspace.name }}</span>
      <span class="active-path">{{ activeWorkspace.display_path }}</span>
    </div>

    <!-- Workspace List -->
    <div
      class="list-container"
      :class="{ 'drag-over': dragOver }"
    >
      <div v-if="workspaces.length === 0 && !scanning" class="empty-state">
        <p>No workspaces found.</p>
        <p class="hint">Click "Quick Scan" or "Full Scan" to start.</p>
      </div>

      <div
        v-for="ws in filteredWorkspaces"
        :key="ws.path"
        class="ws-card"
        :class="{ 'ws-open': ws.is_open, 'ws-active': ws.is_open && ws.name === activeWorkspace?.name }"
        @click="ws.is_open && focusWorkspace(ws.name)"
      >
        <div class="ws-traffic-light" :class="{ open: ws.is_open, active: ws.is_open && ws.name === activeWorkspace?.name }">
          <span v-if="ws.is_open" class="dot" :class="{ active: ws.name === activeWorkspace?.name }"></span>
        </div>
        <div class="ws-icon">
          <VscodeIcon :color="ws.color" :size="28" />
        </div>
        <div class="ws-info">
          <span class="ws-name">{{ ws.name }}</span>
          <span class="ws-path">{{ ws.display_path }}</span>
        </div>
        <div class="ws-actions">
          <button class="ws-btn" title="Peacock color" @click.stop="openColorModal(ws)">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
          </button>
          <button class="ws-btn ws-btn-launch" title="Open in VS Code" @click.stop="launchWorkspace(ws.path)">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
          </button>
        </div>
      </div>
    </div>

    <!-- Footer -->
    <footer class="app-footer">
      <span>v{{ appVersion }}</span>
    </footer>

    <!-- Drop Zone Overlay -->
    <Teleport to="body">
      <div v-if="dragOver" class="drop-zone">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
        <span>Drop folder to add workspace</span>
      </div>
    </Teleport>

    <!-- Color Modal -->
    <Teleport to="body">
      <div
        v-if="modalWs"
        class="modal-overlay"
        @click="closeColorModal"
      >
        <div class="modal" @click.stop>
          <div class="modal-header">
            <span>🎨 {{ modalWs.name }}</span>
            <button class="modal-close" @click="closeColorModal">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>
            </button>
          </div>
          <div class="color-grid">
            <button
              v-for="c in PEAKOCK_COLORS"
              :key="c"
              class="color-swatch"
              :style="{ background: c }"
              :class="{ active: modalWs.color?.toLowerCase() === c.toLowerCase() }"
              :title="c"
              @click="pickColor(c)"
            >
              <svg v-if="modalWs.color?.toLowerCase() === c.toLowerCase()" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
            </button>
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

/* ── Footer ───────────────────────────────────────────── */
.app-footer {
  display: flex;
  justify-content: flex-end;
  padding: 4px 12px;
  border-top: 1px solid #21262d;
  background: #161b22;
  flex-shrink: 0;
  font-size: 11px;
  color: #484f58;
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
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  border: 1px solid #30363d;
  border-radius: 6px;
  font-size: 13px;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
  white-space: nowrap;
  font-family: inherit;
  color: #c9d1d9;
  background: #21262d;
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-primary {
  background: #1f6feb;
  color: #fff;
  border-color: #388bfd;
}

.btn-primary:hover:not(:disabled) {
  background: #388bfd;
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

.search-icon-svg {
  flex-shrink: 0;
  color: #8b949e;
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
  display: flex;
  align-items: center;
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
  scrollbar-width: auto;
  scrollbar-color: #484f58 #161b22;
}

.list-container::-webkit-scrollbar {
  width: 10px;
}
.list-container::-webkit-scrollbar-track {
  background: #161b22;
}
.list-container::-webkit-scrollbar-thumb {
  background: #484f58;
  border-radius: 5px;
  border: 2px solid #161b22;
}
.list-container::-webkit-scrollbar-thumb:hover {
  background: #6e7681;
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
  border: 1px solid transparent;
  cursor: pointer;
}

.ws-card:hover {
  background: #1c2128;
  border-color: #30363d;
}

.ws-card.ws-open {
  background: #0d1a14;
  border-color: #1a3a2a;
}

.ws-card.ws-open:hover {
  background: #111f19;
  border-color: #1f4d33;
}

.ws-card.ws-active {
  background: #0d1f2d;
  border-color: #1a3a5a;
}

.ws-card.ws-active:hover {
  background: #112538;
  border-color: #1f4d7a;
}

/* ── Traffic light ────────────────────────────────────── */
.ws-traffic-light {
  flex-shrink: 0;
  width: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.ws-traffic-light .dot {
  display: block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #2ea043;
  box-shadow: 0 0 6px #2ea04388;
}

.ws-traffic-light .dot.active {
  background: #58a6ff;
  box-shadow: 0 0 8px #58a6ff99;
  animation: pulse-dot 2s ease-in-out infinite;
}

@keyframes pulse-dot {
  0%, 100% { box-shadow: 0 0 6px #58a6ff88; }
  50% { box-shadow: 0 0 12px #58a6ffcc; }
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

.ws-actions {
  flex-shrink: 0;
  display: flex;
  gap: 4px;
  opacity: 0;
  transition: opacity 0.12s;
}

.ws-card:hover .ws-actions {
  opacity: 1;
}

.ws-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: #8b949e;
  cursor: pointer;
  transition: background 0.12s, color 0.12s, border-color 0.12s;
}

.ws-btn:hover {
  background: #30363d;
  color: #c9d1d9;
  border-color: #484f58;
}

.ws-btn-launch:hover {
  color: #58a6ff;
  border-color: #1f6feb;
}

/* ── Active workspace banner ──────────────────────────── */
.active-banner {
  display: flex; align-items: center; gap: 12px;
  padding: 24px 16px 24px 50px; flex-shrink: 0;
  position: relative; overflow: hidden;
  background:
    linear-gradient(90deg, var(--ws-color) 0% 20%, transparent 20%),
    radial-gradient(circle at 20% 99%, 
      var(--ws-color) 0% 15%, var(--ws-color) 19%,
      color-mix(in srgb, var(--ws-color) 85%, #000) 15% 32%, color-mix(in srgb, var(--ws-color) 85%, #000) 36%,
      color-mix(in srgb, var(--ws-color) 65%, #000) 32% 48%, color-mix(in srgb, var(--ws-color) 65%, #000) 52%,
      color-mix(in srgb, var(--ws-color) 45%, #000) 48% 65%, color-mix(in srgb, var(--ws-color) 45%, #000) 69%,
      color-mix(in srgb, var(--ws-color) 25%, #000) 65% 100%
    );
  border-bottom: 1px solid #21262d;
}
.active-name {
  font-weight: 700; font-size: 24px; letter-spacing: 2px;
  text-transform: uppercase; color: #fff;
  text-shadow: 0 1px 4px rgba(0,0,0,0.6);
}
.active-path {
  font-size: 11px; color: rgba(255,255,255,0.7); margin-left: auto;
  text-shadow: 0 1px 3px rgba(0,0,0,0.5);
}

/* ── Drop zone ────────────────────────────────────────── */
.drop-zone {
  position: fixed;
  inset: 0;
  z-index: 9997;
  background: rgba(13, 17, 23, 0.85);
  border: 3px dashed #1f6feb;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: #58a6ff;
  font-size: 18px;
  font-weight: 500;
  pointer-events: none;
  margin: 8px;
  border-radius: 10px;
}

/* ── Modal ────────────────────────────────────────────── */
.modal-overlay {
  position: fixed;
  inset: 0;
  z-index: 9998;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
}

.modal {
  background: #1c2128;
  border: 1px solid #30363d;
  border-radius: 10px;
  padding: 14px 16px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.6);
  min-width: 280px;
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
  font-size: 14px;
  font-weight: 600;
  color: #e6edf3;
}

.modal-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: #8b949e;
  cursor: pointer;
}

.modal-close:hover {
  background: #30363d;
  color: #c9d1d9;
}

.color-grid {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 8px;
}

.color-swatch {
  width: 36px;
  height: 36px;
  border-radius: 6px;
  border: 2px solid transparent;
  cursor: pointer;
  transition: border-color 0.1s, transform 0.1s;
  display: flex;
  align-items: center;
  justify-content: center;
}

.color-swatch:hover {
  border-color: #8b949e;
  transform: scale(1.15);
}

.color-swatch.active {
  border-color: #fff;
  box-shadow: 0 0 10px rgba(255, 255, 255, 0.4);
  transform: scale(1.05);
}
</style>