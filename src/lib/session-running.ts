/** Sidebar / catalog hints that a Pi thread is mid-run. */
export type SessionRunningHints = {
  metaStatus?: string;
  itemStatus?: string;
  isActive?: boolean;
  extrasStatus?: string;
};

/**
 * A session is running if the catalog says so, the aui thread item says so,
 * or this is the open thread and the live runtime extras say so.
 *
 * The last clause is required: GET /threads is polled slowly and cold
 * (other-workspace) rows are synthesized as `idle`, so the active stream
 * would otherwise never show a pulse.
 */
export function isSessionRunning({
  metaStatus,
  itemStatus,
  isActive,
  extrasStatus,
}: SessionRunningHints): boolean {
  if (metaStatus === "running" || itemStatus === "running") return true;
  return Boolean(isActive && extrasStatus === "running");
}
