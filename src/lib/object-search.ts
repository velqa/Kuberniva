/**
 * Case-insensitive search across an object's visible fields. Every word must match some
 * field, so "failed team-01" finds failed Pods in team-01.
 */
export function searchTokens(query: string) {
  return query.trim().toLowerCase().split(/\s+/).filter(Boolean);
}

export function matchesSearch(tokens: string[], fields: (string | number | null | undefined)[]) {
  if (!tokens.length) return true;
  const haystack = fields.filter((field) => field !== undefined && field !== null && field !== '').join(' ').toLowerCase();
  return tokens.every((token) => haystack.includes(token));
}
