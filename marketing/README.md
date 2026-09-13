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
src/components/sections/    hero, stage, quickstart, missions, features,
                            review, gallery, architecture, closer
public/screens/             real app screenshots (window-cropped)
```

Screenshots in `public/screens/` are the real app, cropped to the window (the
colourful desktop wallpapers were trimmed). They are `next/image`-optimized at
runtime; the hero shot is `priority`, the rest lazy-load.

The visual system mirrors the warm.run landing: a 1280px `shell` with hairline
rules down each edge, dashed separators between `band`s, a dark slate page
(`#141518`) with recessed well bands, `#1c1d21` cards, a single blue micro-accent
(`#5b9dff`), and Inter at light weights (250/300/350) with Geist Mono for labels.

Copy and product claims come from the repository root docs (`DESIGN.md`,
`PRODUCT.md`, `README.md`). No claim on this page is invented — feature copy
mirrors what the app actually does today.
