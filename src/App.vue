<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import VscodeIcon from "./components/VscodeIcon.vue";
import UpdateBanner from "./components/UpdateBanner.vue";
import TerminalPanel from "./components/TerminalPanel.vue";
import type { TerminalTab } from "./components/TerminalPanel.vue";
import { useWorkspaceFlip } from "./utils/useWorkspaceFlip";

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

interface TaskItem {
  label: string;
  command: string;
  args: string[];
  cwd: string | null;
  icon: string;
  task_type: string;
  url: string | null;
  confirm_before_run?: boolean;
  close_when_done?: boolean;
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

const sortedWorkspaces = computed(() => {
  const sorted = [...filteredWorkspaces.value];
  sorted.sort((a, b) => {
    // Active workspace first
    const aActive = a.is_open && a.name === activeWorkspace.value?.name ? 0 : 1;
    const bActive = b.is_open && b.name === activeWorkspace.value?.name ? 0 : 1;
    if (aActive !== bActive) return aActive - bActive;
    // Open workspaces next
    const aOpen = a.is_open ? 1 : 2;
    const bOpen = b.is_open ? 1 : 2;
    if (aOpen !== bOpen) return aOpen - bOpen;
    // Alphabetical within same group
    return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
  });
  return sorted;
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

const launching = ref<Record<string, boolean>>({});

async function launchWorkspace(path: string, wsName: string) {
  launching.value[wsName] = true;
  try {
    await invoke("launch_workspace", { path });
  } catch (e) {
    launching.value[wsName] = false;
    statusMessage.value = `Launch error: ${e}`;
  }
}

async function focusWorkspace(name: string) {
  try {
    await invoke("focus_workspace", { name });
    debugActiveName.value = name;
    isRealFocus.value = true;
  } catch (e) {
    statusMessage.value = `Focus error: ${e}`;
  }
}

async function minimizeWorkspace(name: string) {
  try {
    await invoke("minimize_workspace", { name });
    debugActiveName.value = "";
    isRealFocus.value = false;
  } catch (e) {
    statusMessage.value = `Minimize error: ${e}`;
  }
}

function handleRowClick(ws: WorkspaceInfo) {
  if (!ws.is_open) return;
  if (ws.name === activeWorkspace.value?.name) {
    // If truly focused → minimize; if only "remembered" → bring to foreground
    if (isRealFocus.value) {
      minimizeWorkspace(ws.name);
    } else {
      focusWorkspace(ws.name);
    }
  } else {
    focusWorkspace(ws.name);
  }
}

// ── Color Modal ─────────────────────────────────────────
const modalWs = ref<WorkspaceInfo | null>(null);
const promptsEnabled = ref(false);

// ── Task View ──────────────────────────────────────────
const taskView = ref(false);
const taskViewWsName = ref<string | null>(null);
const tasks = ref<TaskItem[]>([]);

// Confirm-before-run modal state
const confirmModalVisible = ref(false);
const confirmModalTask = ref<TaskItem | null>(null);

// Track missing custom icons to fallback to built-in path icon
const missingIcons = ref(new Set<string>());
function iconLoadError(taskType: string | undefined) {
  if (!taskType) return;
  missingIcons.value.add(taskType);
}

const PEAKOCK_COLORS = [
  "#007fff", "#ff007f", "#00bcd4", "#00ff7f", "#9c27b0",
  "#ff5722", "#ffc107", "#3f51b5", "#8bc34a", "#e91e63",
  "#009688", "#607d8b", "#1857a4", "#dd0531", "#832561",
];

async function openColorModal(ws: WorkspaceInfo) {
  modalWs.value = ws;
  try {
    promptsEnabled.value = await invoke("check_prompts_folder", { workspacePath: ws.path });
  } catch {
    promptsEnabled.value = false;
  }
}

async function togglePrompts() {
  const ws = modalWs.value;
  if (!ws) return;
  try {
    promptsEnabled.value = await invoke("toggle_prompts_folder", { workspacePath: ws.path });
    statusMessage.value = promptsEnabled.value
      ? `Prompts folder added to ${ws.name}`
      : `Prompts folder removed from ${ws.name}`;
  } catch (e) {
    statusMessage.value = `Error: ${e}`;
  }
}

async function toggleTaskView(ws: WorkspaceInfo) {
  if (taskView.value) {
    taskView.value = false;
    taskViewWsName.value = null;
    return;
  }
  try {
    const rawTasks: any = await invoke("get_workspace_tasks", { workspacePath: ws.path });
    tasks.value = (rawTasks || []).map((t: any) => ({
      ...t,
      confirm_before_run: !!(
        t.confirm_before_run ||
        (t.codeSpace && (t.codeSpace.confirmationRequest ?? t.codeSpace.confirmationrequest)) ||
        t.confirmationRequest ||
        t.confirmationrequest
      ),
    }));
  } catch {
    tasks.value = [];
  }
  taskViewWsName.value = ws.name;
  taskView.value = true;
}

async function runTaskExecute(task: TaskItem) {
  const ws = activeWorkspace.value;
  const wsName = ws?.name || "";
  if (task.task_type) {
    if (!runningTasks.value[wsName]) runningTasks.value[wsName] = [];
    if (!runningTasks.value[wsName].includes(task.task_type)) {
      runningTasks.value[wsName].push(task.task_type);
    }
  }
  try {
    const tabId = task.task_type + "-" + Date.now();
    const color = ws?.color || "#0078d4";
    if (!terminalTabs.value[wsName]) terminalTabs.value[wsName] = [];
    terminalTabs.value[wsName].push({ id: tabId, label: task.label, taskType: task.task_type || "default", color });
    if (terminalTabs.value[wsName].length === 1) {
      try {
        const win = getCurrentWindow();
        const size = await win.outerSize();
        await win.setSize(new LogicalSize(size.width * 2, size.height));
      } catch (_) {}
    }
    console.log("[task] spawning terminal:", tabId, task.command, task.args);
    await invoke("terminal_spawn", { terminalId: tabId, command: task.command, args: task.args, cwd: task.cwd });
    if (task.url) {
      setTimeout(() => { invoke("launch_url", { url: task.url }).catch(() => {}); }, 2000);
    }
    statusMessage.value = `Running: ${task.label}`;
  } catch (e) {
    statusMessage.value = `Error: ${e}`;
  }
}

function runTask(task: TaskItem) {
  if (task.confirm_before_run) {
    confirmModalTask.value = task;
    confirmModalVisible.value = true;
    return;
  }
  void runTaskExecute(task);
}

function cancelConfirmRun() {
  confirmModalVisible.value = false;
  confirmModalTask.value = null;
}

async function confirmRunTask() {
  const task = confirmModalTask.value;
  if (!task) return cancelConfirmRun();
  confirmModalVisible.value = false;
  confirmModalTask.value = null;
  await runTaskExecute(task);
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

// ── FLIP animation for workspace reordering ──────────────
const listRef = ref<HTMLElement | null>(null);
useWorkspaceFlip(listRef);

// ── Lifecycle ────────────────────────────────────────────
const activeWorkspace = computed(() => {
  // Empty string = user explicitly cleared via minimizeWorkspace
  if (debugActiveName.value === "") return null;
  if (debugActiveName.value) {
    return workspaces.value.find(w => w.name === debugActiveName.value) || null;
  }
  // No active workspace (all minimized, desktop, or initial state)
  return null;
});

let unlistenWs: (() => void) | null = null;
let unlistenLaunched: (() => void) | null = null;
let unlistenLaunchFailed: (() => void) | null = null;
const debugOpenNames = ref<string[]>([]);
const debugActiveName = ref<string | null>(null);
const isRealFocus = ref(false);
const liveTerminals = ref<Record<string, number[]>>({});
const runningTasks = ref<Record<string, string[]>>({});
const terminalTabs = ref<Record<string, TerminalTab[]>>({});

async function toggleLiveTerminal(wsName: string) {
  const hwnds = liveTerminals.value[wsName];
  if (hwnds && hwnds.length > 0) {
    await invoke("toggle_live_terminal", { hwnds });
  }
}

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
  unlistenWs = await listen<{ open_names: string[]; active_name: string | null; live_terminals: Record<string, number[]> }>("workspace-changed", (event) => {
    const { open_names, active_name, live_terminals } = event.payload;
    debugOpenNames.value = open_names;
    // Keep last active when CodeSpace has focus (scrolling etc.)
    if (active_name !== null) {
      debugActiveName.value = active_name;
      isRealFocus.value = true;
    } else if (debugActiveName.value === "") {
      // User explicitly minimized — keep empty (no selection)
      isRealFocus.value = false;
    } else if (debugActiveName.value && !open_names.includes(debugActiveName.value)) {
      // Last active was closed — clear
      debugActiveName.value = null;
      isRealFocus.value = false;
    } else {
      // No active window (desktop, other app) — keep last known but mark as not real focus
      isRealFocus.value = false;
    }
    workspaces.value.forEach((ws) => {
      ws.is_open = open_names.includes(ws.name);
    });
    liveTerminals.value = live_terminals;
    // Close task view if user switched to a different workspace
    if (taskView.value && active_name !== null && taskViewWsName.value !== active_name) {
      taskView.value = false;
      taskViewWsName.value = null;
    }
  });

  // Listen for workspace-launched event (spinner cleanup)
  unlistenLaunched = await listen<string>("workspace-launched", (event) => {
    const wsName = event.payload;
    launching.value[wsName] = false;
    statusMessage.value = `Launched: ${wsName}`;
  });

  // Listen for workspace-launch-failed event (timeout)
  unlistenLaunchFailed = await listen<string>("workspace-launch-failed", (event) => {
    const wsName = event.payload;
    launching.value[wsName] = false;
    statusMessage.value = `Launch timeout: ${wsName}`;
  });

  // Listen for task-finished events (hide running spinner)
  await listen<{ workspaceName: string; taskType: string }>("task-finished", (event) => {
    console.log("[task-finished] received:", event.payload);
    const { workspaceName, taskType } = event.payload;
    const arr = runningTasks.value[workspaceName];
    if (arr) {
      runningTasks.value[workspaceName] = arr.filter(t => t !== taskType);
      if (runningTasks.value[workspaceName].length === 0) {
        delete runningTasks.value[workspaceName];
      }
    }
  });

  // Listen for terminal-exit (just log, tab stays open)
  await listen<{ terminalId: string }>("terminal-exit", (_event) => {
    // Tab stays open so user can see output
  });
  // console.log("[DEBUG] listener set up");
});

onUnmounted(() => {
  if (unlisten) unlisten();
  if (unlistenWs) unlistenWs();
  if (unlistenLaunched) unlistenLaunched();
  if (unlistenLaunchFailed) unlistenLaunchFailed();
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
      <span class="banner-live-slot">
        <button
          v-if="liveTerminals[activeWorkspace.name]?.length"
          class="banner-live-icon"
          title="Toggle live server terminal"
          @click="toggleLiveTerminal(activeWorkspace.name)"
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor" style="position:relative;z-index:1"><path d="M6.34 4.94a1 1 0 0 1 0 1.41 8.5 8.5 0 0 0 0 11.32 1 1 0 0 1-1.41 1.41C1.02 15.18 1.02 8.85 4.93 4.94a1 1 0 0 1 1.41 0zm12.73 0c3.9 3.9 3.9 10.24 0 14.14a1 1 0 0 1-1.41-1.41 8.5 8.5 0 0 0 0-11.32 1 1 0 0 1 1.41-1.41zM9.31 7.81a1 1 0 0 1 0 1.42 4.5 4.5 0 0 0 0 5.54 1 1 0 0 1-1.41 1.41 6.5 6.5 0 0 1 0-8.37 1 1 0 0 1 1.41 0zm6.96 0a6.5 6.5 0 0 1 0 8.37 1 1 0 0 1-1.41-1.41 4.5 4.5 0 0 0 0-5.54 1 1 0 0 1 1.41-1.42zM12.08 10.58a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3z"/></svg>
        </button>
      </span>
      <span class="active-name">{{ activeWorkspace.name }}</span>
      <span class="active-path">{{ activeWorkspace.display_path }}</span>
      <button class="banner-task-btn" :title="taskView ? 'Back to workspaces' : 'Run tasks'" @click="toggleTaskView(activeWorkspace)">
        <svg v-if="taskView" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>
        <svg v-else width="20" height="20" viewBox="0 0 512 512" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path d="M127.083 247.824l50.031-76.906s-74.734-29.688-109.547-3.078C32.755 194.465.005 268.184.005 268.184l37.109 21.516c0-.001 46.969-91.016 89.969-41.876zM264.177 384.918l76.906-50.031s29.688 74.734 3.078 109.547c-26.625 34.797-100.344 67.563-100.344 67.563l-21.5-37.109c-.001 0 91.016-46.97 41.86-95.97zM206.692 362.887l-13.203-13.188c-24 62.375-80.375 49.188-80.375 49.188s-13.188-56.375 49.188-80.375l-13.188-13.188c-34.797-6-79.188 35.984-86.391 76.766C55.536 422.872 54.333 457.654 54.333 457.654s34.781-1.188 75.578-8.391c40.797-7.203 82.781-51.594 76.781-86.376zM505.224 6.777C450.786-18.738 312.927 28.98 236.255 130.668c-58.422 77.453-89.688 129.641-89.688 129.641l46.406 46.406 12.313 12.313 46.391 46.391s52.219-31.25 129.672-89.656C483.005 199.074 530.739 61.215 505.224 6.777zM274.63 237.371c-12.813-12.813-12.813-33.594 0-46.406s33.578-12.813 46.406.016c12.813 12.813 12.813 33.578 0 46.391-12.812 12.813-33.593 12.813-46.406 0zM351.552 160.465c-16.563-16.578-16.563-43.422 0-59.984 16.547-16.563 43.406-16.563 59.969 0s16.563 43.406 0 59.984c-16.563 16.547-43.406 16.547-59.969 0z"/></svg>
      </button>
    </div>

    <!-- Workspace List -->
    <div
      v-if="!taskView"
      class="list-container"
      ref="listRef"
      :class="{ 'drag-over': dragOver }"
    >
      <div v-if="workspaces.length === 0 && !scanning" class="empty-state">
        <p>No workspaces found.</p>
        <p class="hint">Click "Quick Scan" or "Full Scan" to start.</p>
      </div>

      <div
        v-for="ws in sortedWorkspaces"
        :key="ws.path"
        class="ws-card"
        :data-ws-name="ws.name"
        :class="{ 'ws-open': ws.is_open, 'ws-active': ws.is_open && ws.name === activeWorkspace?.name, 'ws-launching': launching[ws.name] }"
        @click="handleRowClick(ws)"
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
          <button class="ws-btn" title="Workspace settings" @click.stop="openColorModal(ws)">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
          </button>
          <div class="ws-launch-slot">
            <button v-if="!launching[ws.name]" class="ws-btn ws-btn-launch" title="Open in VS Code" @click.stop="launchWorkspace(ws.path, ws.name)">
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
            </button>
            <div v-else class="ws-spinner" title="Opening..." role="status">
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
                <path d="M21 12a9 9 0 1 1-6.8-8.7" stroke-dasharray="60" stroke-dashoffset="20">
                  <animateTransform attributeName="transform" type="rotate" from="0 12 12" to="360 12 12" dur="0.8s" repeatCount="indefinite"/>
                </path>
              </svg>
            </div>
          </div>
        </div>
        <div
          v-if="liveTerminals[ws.name]?.length"
          class="ws-live-btn"
          :style="{ '--ws-color': ws.color || '#0078d4' }"
          title="Toggle live server terminal"
          @click.stop="toggleLiveTerminal(ws.name)"
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" style="position:relative;z-index:1"><path d="M6.34 4.94a1 1 0 0 1 0 1.41 8.5 8.5 0 0 0 0 11.32 1 1 0 0 1-1.41 1.41C1.02 15.18 1.02 8.85 4.93 4.94a1 1 0 0 1 1.41 0zm12.73 0c3.9 3.9 3.9 10.24 0 14.14a1 1 0 0 1-1.41-1.41 8.5 8.5 0 0 0 0-11.32 1 1 0 0 1 1.41-1.41zM9.31 7.81a1 1 0 0 1 0 1.42 4.5 4.5 0 0 0 0 5.54 1 1 0 0 1-1.41 1.41 6.5 6.5 0 0 1 0-8.37 1 1 0 0 1 1.41 0zm6.96 0a6.5 6.5 0 0 1 0 8.37 1 1 0 0 1-1.41-1.41 4.5 4.5 0 0 0 0-5.54 1 1 0 0 1 1.41-1.42zM12.08 10.58a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3z"/></svg>
        </div>
        <div
          v-if="runningTasks[ws.name]?.length"
          class="ws-task-spinner"
          :title="'Running: ' + runningTasks[ws.name].join(', ')"
        >
          <svg class="sync-spin-icon" width="24" height="24" viewBox="-4 0 32 32" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path d="M17.594 16h2.031c0-1.813-0.688-3.531-1.75-4.844h-0.031c-0.781-0.938-1.719-1.656-2.844-2.125-0.031 0-0.063-0.031-0.063-0.031-0.188-0.063-0.375-0.188-0.563-0.25-0.063 0-0.094-0.031-0.156-0.031-0.156-0.063-0.375-0.094-0.531-0.125-0.063-0.031-0.156-0.063-0.219-0.063-0.188-0.031-0.344-0.063-0.531-0.094h-0.188c-0.219-0.031-0.469-0.031-0.688-0.031-1.563-0.031-3.094 0.438-4.406 1.344-0.531 0.375-1.344 0.25-1.688-0.281-0.375-0.531-0.219-1.281 0.313-1.656 1.688-1.188 3.656-1.813 5.688-1.813 0.031 0 0.031-0.031 0.031-0.031 0.063 0 0.094 0.031 0.125 0.031 0.281 0 0.563 0 0.813 0.031 0.125 0 0.219 0.031 0.344 0.031 0.125 0.031 0.281 0.031 0.438 0.063 0.063 0 0.125 0.031 0.188 0.031 0.094 0.031 0.25 0.094 0.375 0.125 0.188 0.031 0.406 0.094 0.594 0.156 0.094 0.031 0.188 0.063 0.25 0.094 0.25 0.063 0.5 0.156 0.719 0.25 0.063 0 0.094 0.031 0.125 0.031 1.438 0.625 2.719 1.563 3.75 2.813 0 0.031 0.031 0.031 0.031 0.063 0.156 0.188 0.313 0.406 0.438 0.594 0.031 0 0.031 0.031 0.031 0.063 1.125 1.625 1.813 3.531 1.813 5.656h1.969l-3.188 4.781zM0 16l3.188-4.813 3.219 4.813h-2.031c0 1.781 0.656 3.406 1.719 4.719 0.031 0.031 0.031 0.063 0.063 0.094 0.156 0.188 0.313 0.375 0.469 0.531v0.031c0.5 0.5 1.094 0.938 1.719 1.281 0.031 0 0.031 0.031 0.031 0.031 0.188 0.094 0.406 0.188 0.594 0.25 0.031 0.031 0.094 0.063 0.125 0.063 0.156 0.063 0.313 0.125 0.5 0.188 0.063 0.031 0.125 0.063 0.219 0.063 0.125 0.063 0.313 0.094 0.469 0.125 0.094 0.031 0.188 0.031 0.281 0.063 0.156 0.031 0.313 0.063 0.469 0.063 0.094 0.031 0.156 0.031 0.25 0.031 0.188 0.031 0.438 0.031 0.625 0.031 1.594 0.031 3.125-0.438 4.438-1.375 0.531-0.344 1.344-0.188 1.688 0.344 0.375 0.531 0.219 1.281-0.313 1.656-1.688 1.188-3.656 1.813-5.688 1.813-0.031 0-0.031 0.031-0.031 0.031-0.063 0-0.094-0.031-0.125-0.031-0.25 0-0.531 0-0.813-0.031-0.031 0-0.094-0.031-0.125-0.031-0.125-0.031-0.281-0.063-0.438-0.063-0.063 0-0.125-0.031-0.188-0.031-0.094-0.031-0.25-0.094-0.375-0.125-0.031 0-0.094-0.031-0.125-0.031-0.219-0.063-0.406-0.125-0.594-0.188-0.063-0.031-0.156-0.063-0.219-0.094-0.25-0.094-0.5-0.219-0.719-0.313-0.063-0.031-0.094-0.031-0.125-0.063-1.469-0.688-2.75-1.688-3.781-2.906-0.031-0.031-0.031-0.063-0.063-0.094-1.063-1.313-1.688-2.938-1.688-4.719z"/></svg>
        </div>
      </div>
    </div>

    <!-- Task View + Terminal Split -->
    <div v-show="taskView" class="task-split">
      <div class="task-list-panel">
        <div v-if="tasks.length === 0" class="empty-state">
          <p>No shell tasks found in this workspace.</p>
        </div>
        <div
          v-for="t in tasks"
          :key="t.label"
          class="ws-card"
          @click="runTask(t)"
        >
          <div class="ws-icon">
            <img
              v-if="t.task_type && !missingIcons.has(t.task_type)"
              :src="`/icons/${t.task_type}.svg`"
              width="28"
              height="28"
              alt=""
              @error="() => iconLoadError(t.task_type)"
            />
            <svg v-else width="28" height="28" viewBox="0 0 24 24" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path :d="t.icon"/></svg>
          </div>
          <div class="ws-info">
            <span class="ws-name">{{ t.label }}</span>
            <span v-if="t.task_type === 'live-server'" class="ws-path">live-server</span>
          </div>
        </div>
      </div>
      <TerminalPanel
        :tabs="terminalTabs[taskViewWsName || ''] || []"
        :activeColor="activeWorkspace?.color || '#0078d4'"
        :taskIcon="''"
        :taskType="''"
        @close-tab="(id: string) => { const wsKey = taskViewWsName || ''; const arr = terminalTabs[wsKey]; if (arr) { const i = arr.findIndex(t => t.id === id); if (i !== -1) { invoke('terminal_kill', { terminalId: id }); arr.splice(i, 1); if (arr.length === 0) delete terminalTabs[wsKey]; } } }"
        @select-tab="() => {}"
      />
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

    <!-- Confirm Run Modal -->
    <Teleport to="body">
      <div v-if="confirmModalVisible" class="modal-overlay" @click="cancelConfirmRun">
        <div class="modal" @click.stop>
          <div class="modal-header">
            <span>Confirm Task</span>
            <button class="modal-close" @click="cancelConfirmRun">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>
            </button>
          </div>
          <div class="modal-body" style="padding:12px 16px;">
            <p>Run task: <strong>{{ confirmModalTask && confirmModalTask.label }}</strong>?</p>
            <p style="font-size:12px;color:var(--muted,#8b949e)">This will execute the configured command in the workspace.</p>
          </div>
          <div class="modal-actions" style="display:flex;gap:8px;padding:12px 16px;justify-content:flex-end;">
            <button class="btn" @click="cancelConfirmRun">Cancel</button>
            <button class="btn btn-primary" @click="confirmRunTask">Run</button>
          </div>
        </div>
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
            <span>
              <svg class="modal-palette-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="13.5" cy="6.5" r="1.5" fill="currentColor" stroke="none"/><circle cx="17.5" cy="10.5" r="1.5" fill="currentColor" stroke="none"/><circle cx="8.5" cy="7.5" r="1.5" fill="currentColor" stroke="none"/><circle cx="6.5" cy="12.5" r="1.5" fill="currentColor" stroke="none"/><path d="M12 2C6.49 2 2 6.49 2 12s4.49 10 10 10a2 2 0 0 0 2-2c0-.52-.2-1.01-.57-1.38-.37-.36-.57-.86-.57-1.38 0-1.1.9-2 2-2H16c3.31 0 6-2.69 6-6 0-5.51-4.49-10-10-10Z"/></svg>
              {{ modalWs.name }}
            </span>
            <button class="modal-close" @click="closeColorModal">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>
            </button>
          </div>
          <p class="modal-section-label">Choose a color for this workspace</p>
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
          <div class="modal-prompts-toggle">
            <label class="toggle-label" @click.stop>
              <span class="toggle-text">Include agents &amp; instructions</span>
              <button
                class="toggle-switch"
                :class="{ active: promptsEnabled }"
                @click="togglePrompts"
                :aria-checked="promptsEnabled"
                role="switch"
              >
                <span class="toggle-knob"></span>
              </button>
            </label>
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

/* ── Task + Terminal Split ────────────────────────────── */
.task-split {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.task-list-panel {
  width: 320px;
  flex-shrink: 0;
  overflow-y: auto;
  padding: 4px 8px;
  border-right: 1px solid #3c3c3c;
  scrollbar-width: auto;
  scrollbar-color: #484f58 #161b22;
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
  background: #0d1117;
  will-change: transform;
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
  align-items: center;
  position: relative;
}

.ws-card:hover .ws-actions {
  opacity: 1;
}

/* Keep actions visible during launching, like before */
.ws-card.ws-launching .ws-actions {
  opacity: 1;
}

.ws-live-btn {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border-radius: 6px;
  border: 1px solid rgba(255,255,255,0.2);
  background: color-mix(in srgb, var(--ws-color, #0078d4) 80%, #000);
  cursor: pointer;
  position: relative;
  overflow: hidden;
}

/* Spinner slot shown while launching; kept separate from .ws-actions so gear/settings stays hidden */
.ws-launch-slot {
  flex-shrink: 0;
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
}

.ws-spinner {
  display: flex;
  align-items: center;
  justify-content: center;
  position: absolute;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
}


.ws-live-btn::after {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: 6px;
  background: color-mix(in srgb, var(--ws-color, #0078d4) 25%, #000);
  animation: live-pulse 4s ease-in-out infinite;
}

.ws-live-btn:hover {
  background: color-mix(in srgb, var(--ws-color, #0078d4) 40%, white);
  border-color: rgba(255,255,255,0.4);
}

/* ── Running task spinner ─────────────────────────────── */
.ws-task-spinner {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border-radius: 6px;
  color: #8b949e;
}

.sync-spin-icon {
  animation: sync-spin 3s linear infinite;
}

@keyframes sync-spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

/* ── Active workspace banner ──────────────────────────── */
.active-banner {
  display: flex; align-items: center; gap: 12px;
  padding: 24px 16px 24px 24px; flex-shrink: 0;
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
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  min-width: 0;
}
.active-path {
  font-size: 11px; color: rgba(255,255,255,0.7); margin-left: auto;
  text-shadow: 0 1px 3px rgba(0,0,0,0.5);
  margin-right: 10px;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  flex-shrink: 1;
}

.banner-task-btn {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 42px;
  height: 42px;
  border-radius: 8px;
  border: 1px solid rgba(255,255,255,0.2);
  background: rgba(255,255,255,0.1);
  color: rgba(255,255,255,0.8);
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
}

.banner-task-btn:hover {
  background: rgba(255,255,255,0.2);
  border-color: rgba(255,255,255,0.4);
  color: #fff;
}

.banner-live-slot {
  flex-shrink: 0;
  width: 42px;
  height: 42px;
  margin-right: 0px;
}

.banner-live-icon {
  width: 42px;
  height: 42px;
  border-radius: 8px;
  border: 1px solid rgba(255,255,255,0.2);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--ws-color) 80%, #000);
  color: rgba(255,255,255,0.8);
  position: relative;
  overflow: hidden;
}

.banner-live-icon:hover {
  background: color-mix(in srgb, var(--ws-color) 40%, white);
  border-color: rgba(255,255,255,0.4);
  color: #fff;
}

.banner-live-icon::after {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: 8px;
  background: color-mix(in srgb, var(--ws-color) 25%, #000);
  animation: live-pulse 4s ease-in-out infinite;
}

@keyframes live-pulse {
  0%, 100% { opacity: 0.25; }
  50% { opacity: 0.75; }
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

.modal-section-label {
  font-size: 11px;
  color: #8b949e;
  margin-bottom: 8px;
}

.modal-palette-icon {
  vertical-align: -3px;
  margin-right: 2px;
  color: #8b949e;
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

/* ── Prompts Toggle ───────────────────────────────────── */
.modal-prompts-toggle {
  margin-top: 14px;
  padding-top: 12px;
  border-top: 1px solid #30363d;
}

.toggle-label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  cursor: pointer;
  font-size: 13px;
  color: #c9d1d9;
}

.toggle-text {
  user-select: none;
  flex: 1;
}

.toggle-switch {
  flex-shrink: 0;
  width: 40px;
  height: 22px;
  border-radius: 11px;
  border: none;
  background: #30363d;
  cursor: pointer;
  position: relative;
  transition: background 0.2s;
  padding: 0;
}

.toggle-switch.active {
  background: #58a6ff;
}

.toggle-knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: #c9d1d9;
  transition: transform 0.2s;
}

.toggle-switch.active .toggle-knob {
  transform: translateX(18px);
  background: #fff;
}
</style>