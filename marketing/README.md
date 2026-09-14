# Orbit — marketing site

The landing page for Orbit, the native GPUI workbench for the pi coding agent.
Built with Next.js (App Router), Tailwind CSS v4, and Motion.

## Develop

```bash
npm install
npm run dev      # http://localhost:3000
```

## Build

```bash
npm run build
npm run start
```

## Structure

```
src/app/layout.tsx          fonts (Inter + Geist Mono) + metadata
src/app/globals.css         reference palette + shell/band/dash utilities
src/app/page.tsx            section order
src/components/             nav, footer, ui primitives, screenshot frame
src/components/sections/    hero, stage, steps, workbench, features,
                            guard, review, gallery, closer
src/components/ui/          shadcn primitives (button, card)
public/screens/             real app screenshots (window-cropped)
```

The copy describes the **app** — its features (chat, tools, sessions, find,
providers, plugins, models, usage, appearance) and the steps you take in it
(connect a provider → start a task → watch it work → review → commit), not the
repository's engineering internals.

Screenshots in `public/screens/` are the real app, cropped to the window (the
colourful desktop wallpapers were trimmed). They are `next/image`-optimized at
runtime; the hero shot is `priority`, the rest lazy-load.

The visual system mirrors the warm.run landing: a 1280px `shell` with hairline
rules down each edge, dashed separators between `band`s, a dark slate page
(`#141518`) with recessed well bands, `#1c1d21` cards, a single blue micro-accent
(`#5b9dff`), and Inter at light weights (250/300/350) with Geist Mono for labels.
