<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface TreeNode {
  name: string;
  full_path: string;
  is_workspace: boolean;
  workspace_path: string | null;
  children: TreeNode[];
}

const props = defineProps<{
  nodes: TreeNode[];
  depth: number;
}>();

const emit = defineEmits<{
  launch: [path: string];
}>();

const collapsed = ref<Record<string, boolean>>({});

function toggleCollapse(fullPath: string) {
  collapsed.value[fullPath] = !collapsed.value[fullPath];
}

function isCollapsed(fullPath: string): boolean {
  return collapsed.value[fullPath] ?? false;
}

function onNodeClick(node: TreeNode) {
  if (node.is_workspace && node.workspace_path) {
    emit("launch", node.workspace_path);
  } else {
    toggleCollapse(node.full_path);
  }
}

function getIcon(node: TreeNode): string {
  if (node.is_workspace) {
    return "📁"; // workspace (folder with code)
  }
  // Root drive
  if (node.name.match(/^[A-Z]:\\?$/)) {
    return "💾";
  }
  return isCollapsed(node.full_path) ? "📁" : "📂";
}
</script>

<template>
  <div class="tree-level" :style="{ paddingLeft: depth === 0 ? '0' : '16px' }">
    <div
      v-for="node in nodes"
      :key="node.full_path"
      class="tree-node"
      :class="{
        'is-workspace': node.is_workspace,
        'is-root-drive': depth === 0,
      }"
      @click="onNodeClick(node)"
    >
      <span class="node-icon">{{ getIcon(node) }}</span>
      <span class="node-name">{{ node.name }}</span>
      <span v-if="node.is_workspace" class="node-badge">WS</span>
      <span
        v-if="!node.is_workspace && node.children.length > 0"
        class="node-count"
      >
        {{ node.children.length }}
      </span>
    </div>

    <!-- Children (recursive) -->
    <WorkspaceTree
      v-for="node in nodes"
      v-show="!isCollapsed(node.full_path) && !node.is_workspace"
      :key="'children-' + node.full_path"
      :nodes="node.children"
      :depth="depth + 1"
      @launch="emit('launch', $event)"
    />
  </div>
</template>

<style scoped>
.tree-level {
  user-select: none;
}

.tree-node {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  margin: 1px 0;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.15s;
  font-size: 13px;
}

.tree-node:hover {
  background: rgba(255, 255, 255, 0.06);
}

.tree-node.is-workspace {
  color: #6cc644;
  font-weight: 500;
}

.tree-node.is-workspace:hover {
  background: rgba(108, 198, 68, 0.1);
}

.tree-node.is-root-drive {
  font-weight: 600;
  color: #58a6ff;
}

.node-icon {
  font-size: 16px;
  width: 22px;
  text-align: center;
  flex-shrink: 0;
}

.node-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.node-count {
  font-size: 11px;
  color: #8b949e;
  background: rgba(139, 148, 158, 0.15);
  padding: 1px 6px;
  border-radius: 8px;
}

.node-badge {
  font-size: 10px;
  font-weight: 600;
  color: #1a1a2e;
  background: #6cc644;
  padding: 1px 5px;
  border-radius: 3px;
  text-transform: uppercase;
}
</style>
