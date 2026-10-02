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

export function shouldRecoverAfterResume(hiddenMs: number, lastVerifiedAt: number, now: number, disconnected: boolean) {
  return hiddenMs >= 30_000 || disconnected || (lastVerifiedAt > 0 && now - lastVerifiedAt >= 120_000);
}

/** Exec and auth-provider credentials can open a browser sign-in; give it time (backend waits 300s). */
export function usesInteractiveAuth(authMethod: string | undefined) {
  return authMethod === 'OIDC / exec' || authMethod === 'OIDC provider';
}

export function readDeadlineMs(connecting: boolean, authMethod: string | undefined) {
  if (usesInteractiveAuth(authMethod)) return 310_000;
  return connecting ? 95_000 : 40_000;
}
