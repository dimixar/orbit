import StackIcon, { type IconName } from "tech-stack-icons";

const STACK_BY_KEY: Record<string, IconName> = {
  ts: "typescript",
  tsx: "react",
  js: "js",
  mjs: "js",
  cjs: "js",
  jsx: "react",
  css: "css3",
  scss: "sass",
  sass: "sass",
  less: "css3",
  html: "html5",
  htm: "html5",
  json: "json",
  jsonc: "json",
  xml: "xml",
  md: "markdown",
  mdx: "markdown",
  py: "python",
  go: "go",
  rs: "rust",
  vue: "vuejs",
  php: "php",
  yaml: "yaml",
  yml: "yaml",
  sh: "bash",
  bash: "bash",
  zsh: "zsh",
  sql: "sqlite",
  graphql: "graphql",
  gql: "graphql",
  svelte: "sveltejs",
  java: "java",
  kt: "kotlin",
  kts: "kotlin",
  swift: "swift",
  rb: "ruby",
  cs: "csharp",
  cpp: "c++",
  cc: "c++",
  cxx: "c++",
  h: "c++",
  hpp: "c++",
  docker: "docker",
  gitignore: "git",
  gitattributes: "git",
  dockerfile: "docker",
  svg: "svgo",
  prisma: "prisma",
  toml: "rust",
  lock: "npm",
};

const STACK_BY_BASENAME: Record<string, IconName> = {
  dockerfile: "docker",
  makefile: "c++",
  gemfile: "ruby",
};

export function fileTypeKey(filename: string): string {
  const base = filename.slice(filename.lastIndexOf("/") + 1).toLowerCase();
  if (STACK_BY_BASENAME[base]) return base;
  if (base.startsWith(".") && !base.slice(1).includes(".")) {
    return base.slice(1);
  }
  const dot = base.lastIndexOf(".");
  return dot > 0 ? base.slice(dot + 1) : base;
}

function fallbackFill(key: string): string {
  const hue = [...key].reduce((sum, ch) => sum + ch.charCodeAt(0) * 13, 0) % 360;
  return `oklch(0.74 0.14 ${hue})`;
}

export function FileTypeIcon({
  filename,
  className = "size-4",
}: {
  filename: string;
  className?: string;
}) {
  const key = fileTypeKey(filename);
  const stackName = STACK_BY_BASENAME[key] ?? STACK_BY_KEY[key];
  return (
    <span data-file-type={key} data-slot="avatar" className={className}>
      {stackName ? (
        <StackIcon name={stackName} variant="dark" className="size-full" />
      ) : (
        <svg viewBox="0 0 16 16" aria-hidden className="size-full">
          <path
            fill={fallbackFill(key)}
            d="M3.2 1.8A1.4 1.4 0 0 1 4.6.4h5.1L13.6 4.3v10.3a1.4 1.4 0 0 1-1.4 1.4H4.6a1.4 1.4 0 0 1-1.4-1.4V1.8Z"
          />
          <path fill="#fff" fillOpacity="0.28" d="M9.6.4v4H13.6" />
        </svg>
      )}
    </span>
  );
}
