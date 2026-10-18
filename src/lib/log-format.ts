/**
 * Turns raw Kubernetes log lines (each prefixed with an RFC 3339 timestamp) into readable
 * entries. Kubernetes stamps every physical line, so a pretty-printed dict, JSON object, or
 * stack trace arrives as many separate lines; they are joined back into one entry with one
 * timestamp. Optionally, single-line JSON objects are expanded with indentation.
 */
export type LogLevel = 'error' | 'warn' | 'info';
export type LogEntry = { time: string; stamp: string; text: string; level: LogLevel; lines: number };

const TIMESTAMP = /^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?)\s?(.*)$/;
const MAX_ENTRY_LINES = 400;

export function splitTimestamp(line: string) {
  const match = TIMESTAMP.exec(line);
  return match ? { stamp: match[1], text: match[2] } : { stamp: '', text: line };
}

export function logLevel(text: string): LogLevel {
  return /\b(ERROR|ERR|FATAL|PANIC|CRITICAL)\b|level=(error|fatal)|"level":\s*"(error|fatal|critical)"/i.test(text) ? 'error'
    : /\b(WARN|WARNING)\b|level=warn|"level":\s*"warn(ing)?"/i.test(text) ? 'warn' : 'info';
}

/** Local wall-clock time with milliseconds: fixed width, so entries line up. */
export function shortTime(stamp: string) {
  const time = Date.parse(stamp);
  if (!Number.isFinite(time)) return stamp;
  const date = new Date(time);
  const pad = (value: number, size = 2) => String(value).padStart(size, '0');
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${pad(date.getMilliseconds(), 3)}`;
}

/** Open brackets minus closed ones, ignoring anything inside quotes. */
export function bracketDepth(text: string) {
  let depth = 0;
  let quote = '';
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    if (quote) {
      if (char === '\\') index += 1;
      else if (char === quote) quote = '';
    } else if (char === '"' || char === "'") quote = char;
    else if (char === '{' || char === '[' || char === '(') depth += 1;
    else if (char === '}' || char === ']' || char === ')') depth -= 1;
  }
  return depth;
}

/** A line that belongs to the entry before it: indented, a closing bracket, or a trace frame. */
function continues(text: string, openDepth: number) {
  if (openDepth > 0) return true;
  return /^[\s]+\S/.test(text) || /^[}\])]/.test(text) || /^(Caused by:|\.\.\. \d+ more)/.test(text);
}

export function groupLogLines(lines: readonly string[]): LogEntry[] {
  const entries: LogEntry[] = [];
  let current: LogEntry | null = null;
  let depth = 0;
  for (const line of lines) {
    const { stamp, text } = splitTimestamp(line);
    if (current && current.lines < MAX_ENTRY_LINES && continues(text, depth)) {
      current.text += `\n${text}`;
      current.lines += 1;
      depth = Math.max(0, depth + bracketDepth(text));
      continue;
    }
    current = { stamp, time: stamp ? shortTime(stamp) : '', text, level: logLevel(text), lines: 1 };
    // Only count brackets that are left open at the end of a line (e.g. `payload = {`).
    depth = Math.max(0, bracketDepth(text));
    entries.push(current);
  }
  return entries;
}

/** Rewrites a Python literal (single quotes, True/False/None) as JSON, outside of strings. */
export function pythonToJson(text: string) {
  let out = '';
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    if (char === '"' || char === "'") {
      let value = '';
      let end = index + 1;
      for (; end < text.length && text[end] !== char; end += 1) {
        if (text[end] === '\\' && end + 1 < text.length) {
          end += 1;
          value += text[end] === char ? char : `\\${text[end]}`;
        } else value += text[end];
      }
      out += JSON.stringify(value.replace(/\\(.)/g, (_, next) => (next === 'n' ? '\n' : next === 't' ? '\t' : next)));
      index = end;
      continue;
    }
    const word = (char === 'T' || char === 'F' || char === 'N') && !/\w/.test(text[index - 1] || '') ? /^(True|False|None)\b/.exec(text.slice(index, index + 6)) : null;
    if (word) {
      out += word[1] === 'True' ? 'true' : word[1] === 'False' ? 'false' : 'null';
      index += word[1].length - 1;
      continue;
    }
    out += char;
  }
  return out;
}

/**
 * Pretty-prints an entry that is a single JSON or Python dict/list (optionally after a
 * message prefix); other text is unchanged.
 */
export function prettyJson(text: string) {
  if (text.includes('\n')) return text;
  const trimmed = text.trim();
  const start = trimmed.search(/[{[]/);
  if (start < 0 || !/[}\]]$/.test(trimmed)) return text;
  const body = trimmed.slice(start);
  for (const candidate of [body, pythonToJson(body)]) {
    try {
      const parsed = JSON.parse(candidate);
      if (parsed && typeof parsed === 'object') return `${trimmed.slice(0, start)}${JSON.stringify(parsed, null, 2)}`;
    } catch {
      // Not this format; try the next one.
    }
  }
  return text;
}

export type Highlight = { text: string; match?: number };

/**
 * Splits each text into plain and matching pieces for a case-insensitive search, numbering
 * every match across all texts in order, so the view can step through them.
 */
export function highlightMatches(texts: readonly string[], query: string, limit = 10_000) {
  const needle = query.trim().toLowerCase();
  const pieces: (Highlight[] | null)[] = [];
  let total = 0;
  for (const text of texts) {
    if (!needle || total >= limit) {
      pieces.push(null);
      continue;
    }
    const lower = text.toLowerCase();
    let from = 0;
    let at = lower.indexOf(needle);
    if (at < 0) {
      pieces.push(null);
      continue;
    }
    const parts: Highlight[] = [];
    while (at >= 0 && total < limit) {
      if (at > from) parts.push({ text: text.slice(from, at) });
      parts.push({ text: text.slice(at, at + needle.length), match: total });
      total += 1;
      from = at + needle.length;
      at = lower.indexOf(needle, from);
    }
    if (from < text.length) parts.push({ text: text.slice(from) });
    pieces.push(parts);
  }
  return { pieces, total };
}
