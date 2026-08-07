<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";

const updateVersion = ref("");
const updateAssetId = ref("");
const downloading = ref(false);
const error = ref("");

const CURRENT = "1.0.8";

onMounted(async () => {
  try {
    const release: any = await invoke("check_update");
    const latestTag = release.tag_name.replace("v", "");
    if (compare(latestTag, CURRENT) > 0) {
      updateVersion.value = latestTag;
      const asset = release.assets.find((a: any) => a.name === "CodeSpace.exe");
      if (asset) updateAssetId.value = asset.url;
    }
  } catch (_) {}
});

function compare(a: string, b: string): number {
  const pa = a.split(".").map(Number), pb = b.split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    if ((pa[i] || 0) > (pb[i] || 0)) return 1;
    if ((pa[i] || 0) < (pb[i] || 0)) return -1;
  }
  return 0;
}

async function installUpdate() {
  if (!updateAssetId.value) return;
  downloading.value = true;
  try {
    await invoke("download_and_install", { url: updateAssetId.value });
  } catch (e: any) {
    error.value = `Update failed: ${e}`;
    downloading.value = false;
  }
}
</script>

<template>
  <div v-if="updateVersion" class="update-banner">
    <div class="update-banner-content">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
      <span>Version {{ updateVersion }} available</span>
    </div>
    <div class="update-banner-actions">
      <span v-if="error" class="update-error">{{ error }}</span>
      <button class="btn btn-primary btn-sm" :disabled="downloading" @click="installUpdate">
        {{ downloading ? "Downloading..." : "Update" }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.update-banner { display: flex; align-items: center; justify-content: space-between; padding: 8px 16px; background: #1f6feb; color: #fff; font-size: 13px; flex-shrink: 0; }
.update-banner-content { display: flex; align-items: center; gap: 8px; }
.update-banner-actions { display: flex; align-items: center; gap: 8px; }
.update-error { color: #ffa198; font-size: 12px; }
.btn-sm { padding: 4px 10px; font-size: 12px; background: rgba(255,255,255,0.15); border: 1px solid rgba(255,255,255,0.25); color: #fff; border-radius: 4px; cursor: pointer; }
.btn-sm:hover:not(:disabled) { background: rgba(255,255,255,0.25); }
</style>
