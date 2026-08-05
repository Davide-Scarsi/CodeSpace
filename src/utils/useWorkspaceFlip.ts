import { onBeforeUpdate, onUpdated, type Ref } from "vue";

/**
 * FLIP animation for workspace list reordering.
 * Inspired by Tekna Chem's useTableRowFlip.
 *
 * Usage:
 *   const listRef = ref<HTMLElement | null>(null);
 *   useWorkspaceFlip(listRef);
 *
 * Rows must have `data-ws-name` attribute matching the workspace name.
 */
export function useWorkspaceFlip(listRef: Ref<HTMLElement | null>) {
  let positions = new Map<string, number>();

  onBeforeUpdate(() => {
    if (!listRef.value) return;
    const rows = listRef.value.querySelectorAll("[data-ws-name]");
    positions.clear();
    rows.forEach((row) => {
      const name = (row as HTMLElement).dataset.wsName;
      if (name) {
        positions.set(name, row.getBoundingClientRect().top);
      }
    });
  });

  onUpdated(() => {
    if (!listRef.value) return;
    const rows = listRef.value.querySelectorAll("[data-ws-name]");
    const deltas: { el: HTMLElement; delta: number }[] = [];

    rows.forEach((row) => {
      const el = row as HTMLElement;
      const name = el.dataset.wsName;
      if (!name) return;
      const prev = positions.get(name);
      if (prev === undefined) return;
      const current = el.getBoundingClientRect().top;
      const delta = prev - current;
      if (Math.abs(delta) > 0.5) {
        deltas.push({ el, delta });
      }
    });

    if (deltas.length === 0) return;

    // Invert: apply translate instantly
    for (const { el, delta } of deltas) {
      el.style.transition = "none";
      el.style.transform = `translateY(${delta}px)`;
      el.style.zIndex = "5";
    }

    // Force reflow
    void document.body.offsetHeight;

    // Play: animate to final position
    for (const { el } of deltas) {
      el.style.transition = "transform 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94)";
      el.style.transform = "";
    }

    // Cleanup after transitions finish
    const cleanup = () => {
      for (const { el } of deltas) {
        el.style.transition = "";
        el.style.transform = "";
        el.style.zIndex = "";
      }
    };
    const lastEl = deltas[deltas.length - 1].el;
    lastEl.addEventListener("transitionend", cleanup, { once: true });
  });
}
