/**
 * Composer `/` and `@` tokens — the same grammar as pi's TUI editor.
 *
 *   /skill:name   invoke a skill
 *   /template     expand a prompt template
 *   @path/to/file mention a project file
 */

export const MENTION_RESULT_LIMIT = 40;

export type MentionKind = "slash" | "file";

export type ActiveMention = {
  kind: MentionKind;
  trigger: "/" | "@";
  query: string;
  start: number;
  end: number;
};

export function findActiveMention(
  text: string,
  caret: number,
): ActiveMention | null {
  if (caret < 0 || caret > text.length) return null;
  const before = text.slice(0, caret);
  let tokenStart = before.length - 1;
  while (tokenStart >= 0 && !/\s/.test(before.charAt(tokenStart))) {
    tokenStart -= 1;
  }
  tokenStart += 1;
  const token = before.slice(tokenStart);
  if (token.startsWith("/")) {
    return {
      kind: "slash",
      trigger: "/",
      query: token.slice(1),
      start: tokenStart,
      end: caret,
    };
  }
  if (token.startsWith("@")) {
    return {
      kind: "file",
      trigger: "@",
      query: token.slice(1),
      start: tokenStart,
      end: caret,
    };
  }
  return null;
}

export function replaceMention(
  text: string,
  mention: ActiveMention,
  inserted: string,
): { text: string; caret: number } {
  const next = text.slice(0, mention.start) + inserted + text.slice(mention.end);
  return { text: next, caret: mention.start + inserted.length };
}

export function mentionInsert(trigger: "/" | "@", value: string): string {
  return `${trigger}${value} `;
}

export function scoreMatch(text: string, query: string): number {
  const hay = text.toLowerCase();
  const q = query.toLowerCase();
  if (!q) return 1;
  if (hay === q) return 100;
  if (hay.startsWith(q)) return 80;
  const idx = hay.indexOf(q);
  if (idx >= 0) return 60 - Math.min(idx, 20);
  if (isSubsequence(hay, q)) return 20;
  return 0;
}

function isSubsequence(hay: string, needle: string): boolean {
  let i = 0;
  for (const ch of hay) {
    if (ch === needle[i]) i += 1;
    if (i === needle.length) return true;
  }
  return false;
}

export type CommandLike = {
  kind: "skill" | "prompt";
  name: string;
  description: string;
};

export function filterCommands<T extends CommandLike>(
  commands: T[],
  query: string,
  limit = MENTION_RESULT_LIMIT,
): T[] {
  const q = query.trim().toLowerCase();
  const skills = commands.filter((c) => c.kind === "skill");
  const prompts = commands.filter((c) => c.kind === "prompt");
  if (!q) return [...skills, ...prompts].slice(0, limit);

  const rank = (a: T, b: T) => {
    const sa = Math.max(scoreMatch(a.name, q), scoreMatch(a.description, q) * 0.6);
    const sb = Math.max(scoreMatch(b.name, q), scoreMatch(b.description, q) * 0.6);
    return sb - sa || a.name.localeCompare(b.name);
  };
  const match = (c: T) =>
    scoreMatch(c.name, q) > 0 || scoreMatch(c.description, q) > 0;

  return [...skills.filter(match).sort(rank), ...prompts.filter(match).sort(rank)].slice(
    0,
    limit,
  );
}

export function filterFiles(
  paths: string[],
  query: string,
  limit = MENTION_RESULT_LIMIT,
): string[] {
  const q = query.trim().toLowerCase();
  if (!q) return paths.slice(0, limit);
  return paths
    .map((path) => {
      const base = path.slice(path.lastIndexOf("/") + 1);
      const score = Math.max(scoreMatch(base, q), scoreMatch(path, q) * 0.7);
      return { path, score };
    })
    .filter((row) => row.score > 0)
    .sort((a, b) => b.score - a.score || a.path.localeCompare(b.path))
    .slice(0, limit)
    .map((row) => row.path);
}

export function fileLabel(path: string): { name: string; dir: string } {
  const i = path.lastIndexOf("/");
  if (i < 0) return { name: path, dir: "" };
  return { name: path.slice(i + 1), dir: path.slice(0, i) };
}
