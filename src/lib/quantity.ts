/** Human-readable, rounded labels for Kubernetes resource quantities. */

const DECIMAL: Record<string, number> = { n: 1e-9, u: 1e-6, m: 1e-3, '': 1, k: 1e3, K: 1e3, M: 1e6, G: 1e9, T: 1e12, P: 1e15, E: 1e18 };
const BINARY: Record<string, number> = { Ki: 1024, Mi: 1024 ** 2, Gi: 1024 ** 3, Ti: 1024 ** 4, Pi: 1024 ** 5, Ei: 1024 ** 6 };

/** Parses "250m", "2236230912n", "1846532Ki", "16Gi", "1.5", "2 cores", "1e3". */
export function parseQuantity(value: string | undefined): number | undefined {
  const match = value?.trim().match(/^([+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?)\s*(Ki|Mi|Gi|Ti|Pi|Ei|[numkKMGTPE]|cores?)?$/);
  if (!match) return undefined;
  const number = Number(match[1]);
  if (!Number.isFinite(number)) return undefined;
  const suffix = match[2] || '';
  if (suffix.startsWith('core')) return number;
  return number * (BINARY[suffix] ?? DECIMAL[suffix]);
}

export function formatCores(cores: number) {
  if (cores <= 0) return '0 cores';
  if (cores < 0.001) return '<0.001 cores';
  if (cores < 1) return `${Number(cores.toFixed(cores < 0.1 ? 3 : 2))} cores`;
  return `${Number(cores.toFixed(cores < 10 ? 2 : 1))} ${cores === 1 ? 'core' : 'cores'}`;
}

export function formatBytes(bytes: number) {
  const units: [string, number][] = [['Ti', 1024 ** 4], ['Gi', 1024 ** 3], ['Mi', 1024 ** 2], ['Ki', 1024]];
  for (const [unit, size] of units) {
    if (bytes >= size) {
      const scaled = bytes / size;
      return `${Number(scaled.toFixed(scaled < 100 && (unit === 'Gi' || unit === 'Ti') ? 1 : 0))}${unit}`;
    }
  }
  return `${Math.max(0, Math.round(bytes))}B`;
}

export function cpuLabel(value?: string) {
  const cores = parseQuantity(value);
  return cores === undefined ? value || '—' : formatCores(cores);
}

export function memoryLabel(value?: string) {
  const bytes = parseQuantity(value);
  return bytes === undefined ? value || '—' : formatBytes(bytes);
}

export function percentLabel(percent?: number) {
  if (percent === undefined || !Number.isFinite(percent)) return '—';
  if (percent > 0 && percent < 1) return '<1';
  return String(Math.round(percent));
}

/** Formats node capacity/allocatable entries by resource name; leaves counts like "pods" alone. */
export function resourceQuantityLabel(resource: string, value: string) {
  if (resource === 'cpu') return cpuLabel(value);
  if (resource === 'memory' || resource === 'ephemeral-storage' || resource.startsWith('hugepages-')) return memoryLabel(value);
  return value;
}
