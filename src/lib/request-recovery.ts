/** A stale native request must release its UI state even after the laptop sleeps. */
export async function withRequestDeadline<T>(request: Promise<T>, label: string, timeoutMs = 40_000): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      request,
      new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(new Error(`${label} timed out. Reconnect and try again.`)), timeoutMs);
      }),
    ]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}

export type ResumeState = {
  view: string;
  /** Live watch state for Workloads/Resources: the backend keeps it streaming while the window is hidden. */
  watchStatus: 'idle' | 'connecting' | 'connected' | 'reconnecting' | 'error';
  connectionFailed: boolean;
  /** Age of the visible snapshot for views without a watch (Overview, Events). */
  snapshotAgeMs: number;
};
export type ResumeAction = 'none' | 'quiet-refresh' | 'restart-watch' | 'reconnect';

/**
 * Returning to the window is not a reason to reconnect. Only a real failure reconnects;
 * snapshot views refresh quietly when old; healthy live watches need nothing.
 */
export function resumeAction(state: ResumeState): ResumeAction {
  if (state.connectionFailed) return 'reconnect';
  if (state.view === 'Workloads' || state.view === 'Resources') {
    if (state.watchStatus === 'error') return 'restart-watch';
    return 'none';
  }
  // A streaming feed is already current; only a plain snapshot view refreshes when old.
  if (state.watchStatus === 'connected' || state.watchStatus === 'reconnecting') return 'none';
  if ((state.view === 'Overview' || state.view === 'Events') && state.snapshotAgeMs >= 60_000) return 'quiet-refresh';
  return 'none';
}

/** Exec and auth-provider credentials can open a browser sign-in; give it time (backend waits 300s). */
export function usesInteractiveAuth(authMethod: string | undefined) {
  return authMethod === 'OIDC / exec' || authMethod === 'OIDC provider';
}

export function readDeadlineMs(connecting: boolean, authMethod: string | undefined) {
  if (usesInteractiveAuth(authMethod)) return 310_000;
  return connecting ? 95_000 : 40_000;
}
