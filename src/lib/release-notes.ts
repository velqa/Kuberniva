/**
 * A small, safe reader for Kuberniva's release notes (the GitHub release Markdown).
 * It returns structured blocks for the UI to render as elements, never as HTML.
 */
export type Inline = { text: string; bold?: boolean; code?: boolean };
export type NoteBlock =
  | { type: 'heading'; text: string }
  | { type: 'label'; text: string }
  | { type: 'paragraph'; inline: Inline[] }
  | { type: 'list'; items: Inline[][] };

export function parseInline(text: string): Inline[] {
  const parts: Inline[] = [];
  const pattern = /\*\*(.+?)\*\*|`([^`]+)`/g;
  let last = 0;
  for (const match of text.matchAll(pattern)) {
    if (match.index! > last) parts.push({ text: text.slice(last, match.index) });
    parts.push(match[1] !== undefined ? { text: match[1], bold: true } : { text: match[2], code: true });
    last = match.index! + match[0].length;
  }
  if (last < text.length) parts.push({ text: text.slice(last) });
  return parts;
}

/** In the app, installation instructions and checksums are noise: they are dropped. */
export function parseReleaseNotes(markdown: string): NoteBlock[] {
  const blocks: NoteBlock[] = [];
  let skipping = false;
  for (const raw of markdown.replace(/\r\n/g, '\n').split('\n')) {
    const line = raw.trim();
    if (!line) continue;
    const heading = line.match(/^#{1,6}\s+(.*)$/);
    if (heading) {
      skipping = /updating|install/i.test(heading[1]);
      if (!skipping) blocks.push({ type: 'heading', text: heading[1].trim() });
      continue;
    }
    if (skipping || /^DMG SHA-256:/i.test(line)) continue;
    const bullet = line.match(/^[-*]\s+(.*)$/);
    if (bullet) {
      const previous = blocks[blocks.length - 1];
      if (previous?.type === 'list') previous.items.push(parseInline(bullet[1]));
      else blocks.push({ type: 'list', items: [parseInline(bullet[1])] });
      continue;
    }
    const label = line.match(/^\*\*([^*]+)\*\*$/);
    if (label) {
      blocks.push({ type: 'label', text: label[1] });
      continue;
    }
    blocks.push({ type: 'paragraph', inline: parseInline(line) });
  }
  return blocks;
}
