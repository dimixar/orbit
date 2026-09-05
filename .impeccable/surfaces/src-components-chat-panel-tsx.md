---
version: 1
slug: "src-components-chat-panel-tsx"
primary_target: "src/components/chat-panel.tsx"
related_targets: ["src/components/chat/ComposerMentionMenu.tsx","src/components/chat/ComposerAttach.tsx","src/components/chat/use-composer-mentions.ts","src/components/chat/use-composer-attach.ts","src/lib/composer-mention.ts","src/lib/composer-attach.ts","src/lib/pi-client.ts","agent/sse-server.ts"]
---

# Composer mentions and attachments

Mode: **Operate**. Audience: pi users mid-session — insert a skill, prompt, project file, or image without leaving the composer.

## Structure
- `/` opens skills (`/skill:name`) then prompt templates (`/name`) from pi's DefaultResourceLoader (the same sources the CLI expands).
- `@` fuzzy-lists files from the active workspace (`git ls-files`, gitignored excluded).
- Selection replaces the active token and leaves the caret in the draft — never auto-sends.
- Discoverable from `/` and `@` chips on the composer toolbar; Escape dismisses; Enter/Tab/↑↓ stay on the list while it is open.
- Paperclip (and drag-and-drop / paste) accepts files from any directory. Images become chips the model can see. Other files insert `@path` (workspace-relative when inside the project, absolute otherwise). Images are not also mentioned.

## Data
- `GET /workbench/commands?workspacePath=`
- `GET /workspace/files?workspacePath=`
- Image send: `aui.composer.addAttachment` → Pi `ImageContent` (PNG / JPEG / GIF / WebP, 10 MB, 8 max)

Unresolved: extension-registered slash commands are not listed yet (skills + prompts only, per the request).
