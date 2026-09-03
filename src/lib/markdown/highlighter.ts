/**
 * Shiki syntax highlighting, isolated from React.
 *
 * Uses the JavaScript regex engine (no WASM download) and dual
 * github-light/github-dark themes emitted as CSS variables, so highlighted
 * code adapts to the app's class-based dark mode.
 *
 * Results are cached by `language + code` so unchanged code is never
 * re-highlighted — important for long streaming conversations where the same
 * block re-renders many times.
 */

import { createHighlighterCore, type HighlighterCore } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";

import langBash from "shiki/dist/langs/bash.mjs";
import langCss from "shiki/dist/langs/css.mjs";
import langDiff from "shiki/dist/langs/diff.mjs";
import langGo from "shiki/dist/langs/go.mjs";
import langGraphql from "shiki/dist/langs/graphql.mjs";
import langHtml from "shiki/dist/langs/html.mjs";
import langJava from "shiki/dist/langs/java.mjs";
import langJs from "shiki/dist/langs/javascript.mjs";
import langJson from "shiki/dist/langs/json.mjs";
import langJsx from "shiki/dist/langs/jsx.mjs";
import langKotlin from "shiki/dist/langs/kotlin.mjs";
import langMd from "shiki/dist/langs/markdown.mjs";
import langPhp from "shiki/dist/langs/php.mjs";
import langPython from "shiki/dist/langs/python.mjs";
import langRuby from "shiki/dist/langs/ruby.mjs";
import langRust from "shiki/dist/langs/rust.mjs";
import langSql from "shiki/dist/langs/sql.mjs";
import langSvelte from "shiki/dist/langs/svelte.mjs";
import langSwift from "shiki/dist/langs/swift.mjs";
import langToml from "shiki/dist/langs/toml.mjs";
import langTs from "shiki/dist/langs/ts.mjs";
import langTsx from "shiki/dist/langs/tsx.mjs";
import langVue from "shiki/dist/langs/vue.mjs";
import langYaml from "shiki/dist/langs/yaml.mjs";

import themeGithubDark from "shiki/dist/themes/github-dark.mjs";
import themeGithubLight from "shiki/dist/themes/github-light.mjs";

const LANGUAGES = [
  langBash,
  langCss,
  langDiff,
  langGo,
  langGraphql,
  langHtml,
  langJava,
  langJs,
  langJson,
  langJsx,
  langKotlin,
  langMd,
  langPhp,
  langPython,
  langRuby,
  langRust,
  langSql,
  langSvelte,
  langSwift,
  langToml,
  langTs,
  langTsx,
  langVue,
  langYaml,
];

let highlighterPromise: Promise<HighlighterCore> | null = null;

export function getHighlighter(): Promise<HighlighterCore> {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighterCore({
      themes: [themeGithubLight, themeGithubDark],
      langs: LANGUAGES,
      engine: createJavaScriptRegexEngine(),
    });
  }
  return highlighterPromise;
}

/** Aliases that Shiki's bundled grammars don't resolve on their own. */
const LANGUAGE_ALIASES: Record<string, string> = {
  sh: "bash",
  shell: "bash",
  zsh: "bash",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  ts: "typescript",
  py: "python",
  rb: "ruby",
  rs: "rust",
  yml: "yaml",
  md: "markdown",
  mdx: "markdown",
  txt: "text",
  text: "text",
  plaintext: "text",
  console: "bash",
  dockerfile: "dockerfile",
  ini: "ini",
  toml: "toml",
  gql: "graphql",
  vue: "vue",
  svelte: "svelte",
  html: "html",
  xml: "html",
  svg: "html",
  css: "css",
  scss: "css",
  less: "css",
  json: "json",
  jsonc: "json",
  json5: "json",
  sql: "sql",
  java: "java",
  kotlin: "kotlin",
  swift: "swift",
  go: "go",
  php: "php",
  ruby: "ruby",
  rust: "rust",
  diff: "diff",
  patch: "diff",
};

/** Normalizes a fence language to a Shiki-registered language id. */
export function normalizeLanguage(language?: string): string {
  if (!language) return "text";
  const raw = language.trim().toLowerCase().split(/\s+/)[0] ?? "text";
  return LANGUAGE_ALIASES[raw] ?? raw;
}

const cache = new Map<string, Promise<string>>();
const MAX_CACHE_ENTRIES = 200;

/**
 * Highlights `code` and returns escaped HTML (safe to inject via
 * `dangerouslySetInnerHTML` — Shiki escapes all code content).
 */
export function highlightCode(code: string, language?: string): Promise<string> {
  const lang = normalizeLanguage(language);
  const key = `${lang}\u0000${code}`;
  const cached = cache.get(key);
  if (cached) return cached;

  const promise = getHighlighter().then((highlighter) =>
    highlighter.codeToHtml(code, {
      lang,
      themes: { light: "github-light", dark: "github-dark" },
      defaultColor: false,
    }),
  );

  // Keep the cache bounded for long conversations.
  if (cache.size >= MAX_CACHE_ENTRIES) {
    const oldest = cache.keys().next().value;
    if (oldest !== undefined) cache.delete(oldest);
  }
  cache.set(key, promise);
  return promise;
}

/** Clears the highlight cache (e.g. in tests). */
export function clearHighlightCache(): void {
  cache.clear();
}
