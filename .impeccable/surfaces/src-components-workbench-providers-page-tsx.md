---
version: 1
slug: "src-components-workbench-providers-page-tsx"
primary_target: "src/components/workbench/providers-page.tsx"
related_targets: ["src/lib/pi-client.ts","agent/sse-server.ts"]
---

# Providers page

Mode: **Operate**. Audience: pi users managing where their models come from —
adding OpenAI-compatible gateways, Anthropic-style APIs, or local proxies that
pi doesn't ship with.

## Job

List, add, edit, and remove **custom** model providers; show which providers
are serving models in the live catalog. The only write target is
`~/.pi/agent/models.json` (provider id, name, baseUrl, api, apiKey, models[])
— the same file the pi CLI reads, so edits are shared in both directions.

## Non-negotiables

- **The add flow offers only pi's own supported providers** — sourced live
  from `runtime.getProviders()` via GET /providers (`supported[]`), never a
  hardcoded list. Two-step add: pick from the searchable supported list
  (already-added entries badge "in models.json" and are disabled), then a
  prefilled form.
- A picked supported provider may carry just an API key/base URL (models
  omitted → pi's built-in catalog serves it; row shows "pi catalog").
  Unknown providers require explicit model rows.
- Allowed `api` values derive from the runtime's providers (fallback static
  list when the runtime is down). A picked provider's api is locked — pi
  owns it.
- Built-in providers are pi-managed (`pi /login`) — listed with live model
  counts, never editable here. No fake connect-account flows.
- A broken models.json is surfaced (`parseError`) and blocks saves rather
  than being silently overwritten.
- API keys: typed = replace, blank = keep, explicit checkbox = remove.
- Catalog refresh after save: dispatch `orbit:models-updated` so the model
  picker picks up new providers immediately.

## Direction

Dense workbench list card inheriting the Scoped models page grammar: quiet
status note, provider rows with brand icon + mono id + baseUrl/api/model
count, hover-revealed edit/remove, inline delete confirmation. Add lives in
a right-side sheet: supported-provider picker, then a form mapped 1:1 onto
what pi reads from models.json.

## Unresolved

- OAuth ("radius") providers are not supported by the form — CLI territory.
- models.json comments are dropped on save (JSON.stringify limitation).
