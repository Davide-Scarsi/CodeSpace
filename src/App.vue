<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import WorkspaceTree from "./components/WorkspaceTree.vue";
import type { TreeNode } from "./components/WorkspaceTree.vue";

// ── State ────────────────────────────────────────────────
const treeNodes = ref<TreeNode[]>([]);
const scanInfo = ref({ has_cache: false, count: 0, last_scan: 0 });
const scanning = ref(false);
const searchQuery = ref("");
const statusMessage = ref("");

// ── Computed ─────────────────────────────────────────────
const filteredNodes = computed(() => {
  if (!searchQuery.value.trim()) return treeNodes.value;
  return filterTree(treeNodes.value, searchQuery.value.toLowerCase());
});

function filterTree(nodes: TreeNode[], query: string): TreeNode[] {
  const result: TreeNode[] = [];
  for (const node of nodes) {
    const nameMatch = node.name.toLowerCase().includes(query);
    const filteredChildren = filterTree(node.children, query);
    if (nameMatch || filteredChildren.length > 0) {
      result.push({
        ...node,
        children: filteredChildren.length > 0 ? filteredChildren : node.children,
      });
    }
  }
  return result;
}

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
    treeNodes.value = await invoke("scan_workspaces", { forceFull });
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

// ── Lifecycle ────────────────────────────────────────────
onMounted(async () => {
  await loadScanInfo();
  if (scanInfo.value.has_cache) {
    await scan(false);
  }
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

    <!-- Tree -->
    <div class="tree-container">
      <div v-if="treeNodes.length === 0 && !scanning" class="empty-state">
        <p>No workspaces found.</p>
        <p class="hint">Click "Quick Scan" or "Full Scan" to start.</p>
      </div>
      <WorkspaceTree
        v-else
        :nodes="filteredNodes"
        :depth="0"
        @launch="launchWorkspace"
      />
    </div>
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

/* ── Tree container ───────────────────────────────────── */
.tree-container {
  flex: 1;
  overflow: auto;
  padding: 8px 0;
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
</style>