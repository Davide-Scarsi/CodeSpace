<script setup lang="ts">
import { ref, onMounted } from "vue";
import { check, Update } from "@tauri-apps/plugin-updater";

const update = ref<Update | null>(null);
const downloading = ref(false);
const progress = ref(0);
const error = ref("");

onMounted(async () => {
  try {
    const u = await check();
    if (u?.available) {
      update.value = u;
    }
  } catch (_) {
    // Offline or endpoint not available — ignore
  }
});

async function installUpdate() {
  if (!update.value) return;
  downloading.value = true;
  error.value = "";
  try {
    await update.value.downloadAndInstall((e) => {
      if (e.event === "Started") {
        progress.value = 0;
      } else if (e.event === "Progress") {
        progress.value = Math.round((e.data as any).progress ?? 0);
      }
    });
  } catch (e) {
    error.value = `Update failed: ${e}`;
  } finally {
    downloading.value = false;
  }
}
</script>

<template>
  <div v-if="update" class="update-banner">
    <div class="update-banner-content">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
        <polyline points="7 10 12 15 17 10"/>
        <line x1="12" y1="15" x2="12" y2="3"/>
      </svg>
      <span>Version {{ update.version }} available</span>
      <span v-if="update.body" class="update-notes">{{ update.body }}</span>
    </div>
    <div class="update-banner-actions">
      <span v-if="error" class="update-error">{{ error }}</span>
      <template v-else>
        <span v-if="downloading" class="update-progress">{{ progress }}%</span>
        <button class="btn btn-primary btn-sm" :disabled="downloading" @click="installUpdate">
          {{ downloading ? "Downloading..." : "Update" }}
        </button>
      </template>
    </div>
  </div>
</template>

<style scoped>
.update-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 16px;
  background: #1f6feb;
  color: #fff;
  font-size: 13px;
  flex-shrink: 0;
}

.update-banner-content {
  display: flex;
  align-items: center;
  gap: 8px;
}

.update-notes {
  color: rgba(255, 255, 255, 0.7);
  font-size: 12px;
}

.update-banner-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.update-error {
  color: #ffa198;
  font-size: 12px;
}

.update-progress {
  font-variant-numeric: tabular-nums;
}

.btn-sm {
  padding: 4px 10px;
  font-size: 12px;
  background: rgba(255, 255, 255, 0.15);
  border: 1px solid rgba(255, 255, 255, 0.25);
  color: #fff;
}

.btn-sm:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.25);
}
</style>
