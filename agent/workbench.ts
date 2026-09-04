/**
 * Workbench aggregators — read-only views over pi's on-disk data
 * (~/.pi/agent/sessions, skills, settings.json), shared by both servers:
 *
 *   - agent/index.ts      (WebSocket daemon — sends `usage`/`skills`/`plugins`)
 *   - agent/sse-server.ts (HTTP — GET /workbench/usage|skills|plugins)
 *
 * The pi-coding-agent SDK stays behind the Node boundary; these helpers only
 * read session files and settings — no SDK state is touched.
 */
import path from "node:path";
import fs from "node:fs";
import os from "node:os";

export const PI_DIR = path.join(os.homedir(), ".pi", "agent");
export const SESSIONS_DIR = path.join(PI_DIR, "sessions");

// ---------- Types ----------

export type UsageDay = {
  date: string; // YYYY-MM-DD
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  cost: number;
  sessions: number;
};

export type UsageModel = {
  model: string;
  provider: string;
  input: number;
  output: number;
  cost: number;
  calls: number;
};

/** Per-model-per-day slice (used to compute recent-window per-model usage). */
export type UsageModelDay = {
  date: string;
  provider: string;
  model: string;
  input: number;
  output: number;
  cost: number;
  calls: number;
};

export type UsageReport = {
  totalInput: number;
  totalOutput: number;
  totalCacheRead: number;
  totalCacheWrite: number;
  totalCost: number;
  totalSessions: number;
  totalCalls: number;
  byModel: UsageModel[];
  byDay: UsageDay[];
  /** Per-model totals restricted to the last 30 days (by entry timestamp). */
  recentByModel: UsageModel[];
};

export type SkillInfo = {
  name: string;
  description: string;
  path: string;
  userInvocable: boolean;
  triggers: string[];
};

export type PluginInfo = {
  packages: string[];
  enabledModels: string[];
  extensionsDir: string;
  extensionCount: number;
};

// ---------- Skills ----------

/** Parse YAML-ish frontmatter --- ... --- from a SKILL.md file. */
function parseFrontmatter(text: string): Record<string, string> {
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!m) return {};
  const out: Record<string, string> = {};
  for (const line of m[1].split(/\r?\n/)) {
    const kv = line.match(/^([a-zA-Z0-9_-]+):\s*(.*)$/);
    if (kv) out[kv[1].trim()] = kv[2].trim().replace(/^["']|["']$/g, "");
  }
  return out;
}

export async function listSkills(): Promise<SkillInfo[]> {
  const skillsDir = path.join(PI_DIR, "skills");
  const out: SkillInfo[] = [];
  let entries: fs.Dirent[] = [];
  try {
    entries = await fs.promises.readdir(skillsDir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const e of entries) {
    // Entries may be symlinks (pi installs skills as links) — stat follows them.
    let isDir = e.isDirectory();
    if (!isDir) {
      try {
        isDir = (await fs.promises.stat(path.join(skillsDir, e.name))).isDirectory();
      } catch {
        continue; // broken link
      }
    }
    if (!isDir) continue;
    const skillPath = path.join(skillsDir, e.name, "SKILL.md");
    try {
      const text = await fs.promises.readFile(skillPath, "utf8");
      const fm = parseFrontmatter(text);
      out.push({
        name: fm.name || e.name,
        description: fm.description || "",
        path: skillPath,
        userInvocable: fm["user-invocable"] !== "false",
        triggers: (fm.triggers || "")
          .split(",")
          .map((t) => t.trim())
          .filter(Boolean),
      });
    } catch {
      // No SKILL.md — skip
    }
  }
  out.sort((a, b) => a.name.localeCompare(b.name));
  return out;
}

// ---------- Plugins ----------

async function readJson<T>(file: string): Promise<T | null> {
  try {
    return JSON.parse(await fs.promises.readFile(file, "utf8")) as T;
  } catch {
    return null;
  }
}

export async function listPlugins(): Promise<PluginInfo> {
  const settingsPath = path.join(PI_DIR, "settings.json");
  const settings = (await readJson<{ packages?: string[]; enabledModels?: string[] }>(
    settingsPath,
  )) ?? {};
  const extensionsDir = path.join(PI_DIR, "extensions");
  let extensionCount = 0;
  try {
    extensionCount = (await fs.promises.readdir(extensionsDir)).filter(
      (f) => f.endsWith(".ts") || f.endsWith(".js"),
    ).length;
  } catch {
    // dir may not exist
  }
  return {
    packages: settings.packages ?? [],
    enabledModels: settings.enabledModels ?? [],
    extensionsDir,
    extensionCount,
  };
}

// ---------- Usage ----------

/**
 * Aggregate usage across every session file. Sessions are append-only JSONL;
 * assistant "message" entries carry a usage object. Per-file aggregates are
 * cached and keyed on mtime+size, so repeat requests only re-read files that
 * grew. Malformed lines are skipped; ~166 MB of history aggregates in a few
 * seconds on first load, instantly afterwards.
 */
type FileUsage = {
  mtimeMs: number;
  size: number;
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  cost: number;
  calls: number;
  byModel: [string, UsageModel][];
  byDay: [string, UsageDay][];
  byModelDay: [string, UsageModelDay][];
};

const usageCache = new Map<string, FileUsage>();

async function parseSessionFile(file: string, stat: fs.Stats): Promise<FileUsage> {
  const agg: FileUsage = {
    mtimeMs: stat.mtimeMs,
    size: stat.size,
    input: 0, output: 0, cacheRead: 0, cacheWrite: 0, cost: 0, calls: 0,
    byModel: [], byDay: [], byModelDay: [],
  };
  const modelMap = new Map<string, UsageModel>();
  const dayMap = new Map<string, UsageDay>();
  const modelDayMap = new Map<string, UsageModelDay>();
  let text = "";
  try {
    text = await fs.promises.readFile(file, "utf8");
  } catch {
    return agg;
  }
  let lastModel = "unknown";
  let lastProvider = "unknown";
  for (const line of text.split("\n")) {
    // Cheap pre-filter: skip lines that can't carry model or usage info.
    if (!line.includes('"usage":{') && !line.includes('"model_change"')) continue;
    let entry: any;
    try {
      entry = JSON.parse(line);
    } catch {
      continue;
    }
    if (entry.type === "model_change") {
      lastModel = entry.modelId ?? lastModel;
      lastProvider = entry.provider ?? lastProvider;
      continue;
    }
    if (entry.type !== "message") continue;
    const u = entry.message?.usage;
    if (!u) continue;
    const input = Number(u.input) || 0;
    const output = Number(u.output) || 0;
    agg.input += input;
    agg.output += output;
    agg.cacheRead += Number(u.cacheRead) || 0;
    agg.cacheWrite += Number(u.cacheWrite) || 0;
    agg.cost += Number(u.cost?.total) || 0;
    agg.calls += 1;

    const mk = `${lastProvider}/${lastModel}`;
    const mm = modelMap.get(mk) ?? {
      model: lastModel, provider: lastProvider, input: 0, output: 0, cost: 0, calls: 0,
    };
    mm.input += input; mm.output += output;
    mm.cost += Number(u.cost?.total) || 0;
    mm.calls += 1;
    modelMap.set(mk, mm);

    const day = String(entry.timestamp ?? "").slice(0, 10);
    if (day) {
      const dd = dayMap.get(day) ?? {
        date: day, input: 0, output: 0, cacheRead: 0, cacheWrite: 0, cost: 0, sessions: 0,
      };
      dd.input += input; dd.output += output;
      dd.cacheRead += Number(u.cacheRead) || 0;
      dd.cacheWrite += Number(u.cacheWrite) || 0;
      dd.cost += Number(u.cost?.total) || 0;
      dayMap.set(day, dd);

      // Per-model-per-day: lets getUsage() compute recent-window per-model usage.
      const mdk = `${day}|${mk}`;
      const md = modelDayMap.get(mdk) ?? {
        date: day, provider: lastProvider, model: lastModel,
        input: 0, output: 0, cost: 0, calls: 0,
      };
      md.input += input; md.output += output;
      md.cost += Number(u.cost?.total) || 0;
      md.calls += 1;
      modelDayMap.set(mdk, md);
    }
  }
  agg.byModel = [...modelMap.entries()].map(([, v]) => [`${v.provider}/${v.model}`, v]);
  agg.byDay = [...dayMap.entries()].map(([, v]) => [v.date, v]);
  agg.byModelDay = [...modelDayMap.entries()].map(([k, v]) => [k, v]);
  return agg;
}

export async function getUsage(): Promise<UsageReport> {
  const report: UsageReport = {
    totalInput: 0, totalOutput: 0, totalCacheRead: 0, totalCacheWrite: 0,
    totalCost: 0, totalSessions: 0, totalCalls: 0, byModel: [], byDay: [], recentByModel: [],
  };
  const modelMap = new Map<string, UsageModel>();
  const dayMap = new Map<string, UsageDay>();
  const modelDayMap = new Map<string, UsageModelDay>();

  let projectDirs: fs.Dirent[] = [];
  try {
    projectDirs = await fs.promises.readdir(SESSIONS_DIR, { withFileTypes: true });
  } catch {
    return report;
  }

  // Collect every session file across projects.
  const files: string[] = [];
  for (const dir of projectDirs) {
    if (!dir.isDirectory()) continue;
    const dirPath = path.join(SESSIONS_DIR, dir.name);
    try {
      for (const f of await fs.promises.readdir(dirPath)) {
        if (f.endsWith(".jsonl")) files.push(path.join(dirPath, f));
      }
    } catch {
      // unreadable dir — skip
    }
  }
  report.totalSessions = files.length;

  // Parse (or reuse cached aggregates) with bounded concurrency.
  const CONCURRENCY = 16;
  for (let i = 0; i < files.length; i += CONCURRENCY) {
    const batch = files.slice(i, i + CONCURRENCY);
    const results = await Promise.all(
      batch.map(async (file) => {
        let stat: fs.Stats;
        try {
          stat = await fs.promises.stat(file);
        } catch {
          return null;
        }
        const cached = usageCache.get(file);
        if (cached && cached.mtimeMs === stat.mtimeMs && cached.size === stat.size) {
          return cached;
        }
        const fresh = await parseSessionFile(file, stat);
        usageCache.set(file, fresh);
        return fresh;
      }),
    );
    for (const agg of results) {
      if (!agg) continue;
      report.totalInput += agg.input;
      report.totalOutput += agg.output;
      report.totalCacheRead += agg.cacheRead;
      report.totalCacheWrite += agg.cacheWrite;
      report.totalCost += agg.cost;
      report.totalCalls += agg.calls;
      for (const [mk, mm] of agg.byModel) {
        const cur = modelMap.get(mk) ?? { ...mm, input: 0, output: 0, cost: 0, calls: 0 };
        cur.input += mm.input; cur.output += mm.output; cur.cost += mm.cost; cur.calls += mm.calls;
        modelMap.set(mk, cur);
      }
      for (const [dk, dd] of agg.byDay) {
        const cur = dayMap.get(dk) ?? {
          date: dk, input: 0, output: 0, cacheRead: 0, cacheWrite: 0, cost: 0, sessions: 0,
        };
        cur.input += dd.input; cur.output += dd.output;
        cur.cacheRead += dd.cacheRead; cur.cacheWrite += dd.cacheWrite;
        cur.cost += dd.cost;
        dayMap.set(dk, cur);
      }
      for (const [k, md] of agg.byModelDay ?? []) {
        const cur = modelDayMap.get(k);
        if (cur) {
          cur.input += md.input; cur.output += md.output;
          cur.cost += md.cost; cur.calls += md.calls;
        } else {
          modelDayMap.set(k, { ...md });
        }
      }
    }
  }

  report.byModel = [...modelMap.values()].sort((a, b) => b.cost - a.cost);
  report.byDay = [...dayMap.values()].sort((a, b) => a.date.localeCompare(b.date));

  // Per-model usage over the last 30 days, from the model×day slices.
  const cutoff = new Date(Date.now() - 30 * 86_400_000).toISOString().slice(0, 10);
  const recentModelMap = new Map<string, UsageModel>();
  for (const md of modelDayMap.values()) {
    if (md.date < cutoff) continue;
    const mk = `${md.provider}/${md.model}`;
    const cur = recentModelMap.get(mk) ?? {
      model: md.model, provider: md.provider, input: 0, output: 0, cost: 0, calls: 0,
    };
    cur.input += md.input; cur.output += md.output; cur.cost += md.cost; cur.calls += md.calls;
    recentModelMap.set(mk, cur);
  }
  report.recentByModel = [...recentModelMap.values()].sort(
    (a, b) => (b.input + b.output) - (a.input + a.output),
  );
  return report;
}