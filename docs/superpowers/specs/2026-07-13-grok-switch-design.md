# Grok Switch Design Spec

**Date:** 2026-07-13  
**Status:** Draft for implementation  
**Product:** Grok Switch — local desktop switcher for Grok CLI (CC Switch equivalent for Grok)

---

## 1. Goal

Build a **local Windows-first desktop app** that lets the user manage and switch:

1. **Relay Providers** — third-party OpenAI/Anthropic-compatible gateways (base URL + API key + models)
2. **Official Grok accounts** — multiple `auth.json` sessions from `grok login`

Switching must immediately affect the installed Grok CLI by writing:

- `~/.grok/config.toml` (providers / default model)
- `~/.grok/auth.json` (official sessions)

All credentials stay on-device. No cloud sync.

### Non-goals (MVP)

- Proxy / failover queues / usage billing (CC Switch advanced features)
- Skill / MCP sync
- Mutating global `ANTHROPIC_*` / `XAI_*` environment variables for Claude Code
- Embedded OAuth browser for `grok login`
- macOS/Linux packaging as first priority (code must remain portable)

---

## 2. Users & success criteria

**Primary user:** same person who uses CC Switch for Claude Code and wants the same workflow for Grok CLI.

**MVP acceptance:**

1. Add a provider (e.g. myallapi) → Enable → `grok -p "ok" -m <gs-model-id>` succeeds against the relay.
2. Import ≥1 provider from `~/.cc-switch/cc-switch.db`.
3. Capture current official `auth.json` as an account → switch away and back; file content restores correctly.
4. Every switch creates a timestamped backup; user can restore last backup.
5. Health check works for at least `chat_completions` and `messages` backends.
6. App shows current mode (provider vs official) and current name on Overview + tray tooltip.

---

## 3. Architecture

```
┌──────────────────────────────────────────┐
│ UI (React + Vite)                        │
│ Overview / Providers / Accounts /        │
│ Import / Activity / Settings             │
└──────────────────┬───────────────────────┘
                   │ Tauri invoke commands
┌──────────────────▼───────────────────────┐
│ Core (Rust, Tauri backend)               │
│ provider_store · account_store           │
│ config_writer · auth_vault               │
│ health · ccswitch_import · backup        │
└──────────────────┬───────────────────────┘
                   │ FS / HTTP
┌──────────────────▼───────────────────────┐
│ ~/.grok/config.toml                      │
│ ~/.grok/auth.json                        │
│ ~/.grok/bin/grok.exe                     │
│ ~/.grok-switch/*                         │
│ ~/.cc-switch/cc-switch.db (read-only)    │
└──────────────────────────────────────────┘
```

### Stack (optimal)

| Layer | Choice | Why |
|-------|--------|-----|
| Shell | **Tauri 2** | Small binary, native FS, tray, like CC Switch class of apps |
| UI | **React + TypeScript + Vite** | Reuse / evolve existing `grok-switch` frontend |
| Styling | CSS modules / single polished stylesheet (existing aesthetic) | No heavy UI kit required for MVP |
| Persistence | **JSON files** under `~/.grok-switch/` | Small data, easy debug; SQLite deferred |
| Config format | **TOML edit** via Rust (`toml_edit`) | Preserve unknown keys/comments when possible |
| HTTP health | `reqwest` | Simple connectivity probes |

### Path conventions

| Purpose | Path |
|---------|------|
| App root | `~/.grok-switch/` |
| Providers | `~/.grok-switch/providers.json` |
| Settings | `~/.grok-switch/settings.json` |
| Activity log | `~/.grok-switch/activity.jsonl` |
| Official account vault | `~/.grok-switch/accounts/<id>/auth.json` + `meta.json` |
| Backups | `~/.grok-switch/backups/<yyyyMMdd-HHmmss>/` |
| Grok home (default) | `~/.grok/` |
| Grok config | `~/.grok/config.toml` |
| Grok auth | `~/.grok/auth.json` |
| Grok binary (default Win) | `~/.grok/bin/grok.exe` |
| CC Switch DB | `~/.cc-switch/cc-switch.db` |

`~` resolves to the user profile (`C:\Users\<name>` on Windows).

---

## 4. Data model

### 4.1 Provider

```ts
type ApiBackend = "chat_completions" | "responses" | "messages";

type ModelEntry = {
  id: string;              // section key WITHOUT prefix, e.g. "myallapi-grok45"
                           // written as [model.gs-<id>]
  model: string;           // API model id, e.g. "grok-4.5"
  name: string;            // UI label
  contextWindow?: number;
  apiBackend?: ApiBackend; // override provider default
};

type Provider = {
  id: string;              // uuid
  name: string;
  baseUrl: string;         // normalized, preferably ending with /v1
  apiKey: string;
  apiBackend: ApiBackend;  // default: chat_completions
  defaultModelEntryId: string; // ModelEntry.id used as [models].default target
  models: ModelEntry[];
  extraHeaders?: Record<string, string>;
  contextWindow: number;   // default 200000
  websiteUrl?: string;
  notes?: string;
  source: "manual" | "cc-switch";
  createdAt: number;
  updatedAt: number;
};
```

**Storage:** `providers.json` = `{ version: 1, items: Provider[] }`.

### 4.2 Account (official)

```ts
type Account = {
  id: string;
  name: string;
  email?: string;
  labelColor: string;
  status: "ready" | "active" | "expired" | "unknown";
  lastUsedAt?: number;
  createdAt: number;
  // files: accounts/<id>/auth.json (raw Grok auth blob)
};
```

**Storage:** directory per account + index in `accounts/index.json`.

### 4.3 Settings

```ts
type Settings = {
  grokHome: string;                 // default ~/.grok
  grokExecutable: string;           // default ~/.grok/bin/grok.exe
  currentMode: "provider" | "official" | "none";
  currentProviderId?: string;
  currentAccountId?: string;
  officialDefaultModel: string;     // default "grok-build"
  autoBackup: boolean;              // default true
  autoHealthCheck: boolean;         // default true
  launchOnStartup: boolean;         // default false
  theme: "system" | "dark" | "light";
  trayEnabled: boolean;             // default true
};
```

### 4.4 Activity

```ts
type Activity = {
  ts: number;
  type:
    | "switch_provider"
    | "switch_account"
    | "import"
    | "health"
    | "backup"
    | "restore"
    | "error"
    | "capture_account";
  message: string;
  meta?: Record<string, string>; // never full secrets
};
```

Append-only JSONL; UI shows last 200 lines.

### 4.5 Secrets policy

- Stored plaintext under `~/.grok-switch/` (same trust model as CC Switch local DB).
- UI masks keys: first 6–8 + last 4 chars.
- Activity / export / logs never include full API keys or full auth tokens.
- Future: Windows Credential Manager (post-MVP).

---

## 5. Grok CLI integration contract

### 5.1 Auth precedence (Grok official)

1. Per-model `api_key` / `env_key` in `config.toml`
2. Session in `auth.json`
3. Env `XAI_API_KEY`

Therefore:

- **Provider mode** must set per-model credentials (wins over official session).
- **Official mode** must point `[models].default` at a model **without** Grok Switch–managed `api_key` (built-in `grok-build` or user-configured official default). Managed `gs-*` sections may remain in file for quick re-switch but must not be the default.

### 5.2 Managed model section naming

All Grok Switch–written model tables use prefix:

```
[model.gs-<ModelEntry.id>]
```

Example: `[model.gs-myallapi-grok45]`

On provider switch:

1. Remove **all** existing tables whose key starts with `gs-` (or `model.gs-`).
2. Write fresh `gs-*` tables for the selected provider’s `models[]`.
3. Set:

```toml
[models]
default = "gs-<defaultModelEntryId>"
```

### 5.3 Provider write template

**chat_completions / responses:**

```toml
[model.gs-example-grok]
model = "grok-4.5"
base_url = "https://relay.example.com/v1"
name = "Grok 4.5 (example)"
api_key = "sk-..."
api_backend = "chat_completions"
context_window = 1000000
```

**messages (Anthropic-compatible relays):**

```toml
[model.gs-example-claude-style]
model = "grok-4.5"
base_url = "https://relay.example.com/v1"
name = "Grok via Messages"
api_backend = "messages"
context_window = 1000000
extra_headers = { "x-api-key" = "sk-...", "anthropic-version" = "2023-06-01" }
```

Notes:

- Prefer **not** setting Bearer `api_key` for `messages` backend; use `extra_headers`.
- `base_url` normalization: strip trailing slash; if user pastes origin without `/v1`, offer auto-append `/v1` in UI (toggle “Append /v1”, default on for new providers).

### 5.4 TOML merge rules

- Use `toml_edit` to preserve unrelated keys and formatting when feasible.
- Never delete non-`gs-*` model tables.
- Only mutate:
  - `[models].default`
  - all `[model.gs-*]` tables
- On first run, if `config.toml` missing, create minimal file with `[cli]` defaults + managed blocks.
- If TOML parse fails: **abort write**, surface error, do not clobber; suggest restore from backup.

### 5.5 Official account switch

1. Backup `auth.json` + `config.toml`.
2. Copy `accounts/<id>/auth.json` → `~/.grok/auth.json`.
3. Set `[models].default = settings.officialDefaultModel` (default `grok-build`).
4. Leave `gs-*` tables in place (inactive).
5. Update settings mode/ids + activity.

### 5.6 Capture official account

- Button: **Capture current Grok session**
- Requires existing `~/.grok/auth.json`
- Copies into new account vault; user names it
- Does not change current mode unless user then enables it

Guided login (MVP): show instructions to run `grok login` / `grok login --device-auth`, then Capture.

---

## 6. Core flows

### 6.1 Enable Provider

```
validate → optional health check → backup → rewrite gs-* + default
→ settings.currentMode=provider → activity → optional post health → toast
```

Health failure: dialog “仍要启用 / 取消”. If force, still write config.

### 6.2 Enable Account

```
validate snapshot exists → backup → copy auth → set official default model
→ settings.currentMode=official → activity → toast
```

### 6.3 Import from CC Switch

Read-only open `~/.cc-switch/cc-switch.db`.

Source rows: `providers` where `app_type` in (`claude`, …) and env contains `ANTHROPIC_BASE_URL` + token.

Mapping:

| CC Switch env | Grok Switch field |
|---------------|-------------------|
| `ANTHROPIC_BASE_URL` | `baseUrl` (normalize) |
| `ANTHROPIC_AUTH_TOKEN` | `apiKey` |
| `ANTHROPIC_DEFAULT_SONNET_MODEL` or `ANTHROPIC_MODEL` | seed `defaultModel` |
| provider `name` / `website_url` | `name` / `websiteUrl` |

Model name sanitize: strip trailing `[1M]` / `[…]` context suffixes.

Default `apiBackend`:

- Heuristic: if user previously used it only via Claude Code Anthropic protocol → default **`messages`**
- Allow override per provider in Import preview and Edit form
- Also expose “Duplicate as OpenAI backend” later; MVP: one backend per import, user can edit after

Dedup: skip if same `baseUrl` + `apiKey` already exists (compare normalized URL + full key hash).

UI: checklist preview → Import selected → `source: "cc-switch"`.

### 6.4 Health check

| Backend | Probe |
|---------|-------|
| `chat_completions` | `GET {base}/models` with `Authorization: Bearer`; on fail `POST {base}/chat/completions` minimal body |
| `responses` | `POST {base}/responses` minimal |
| `messages` | `POST {base}/messages` with `x-api-key` + `anthropic-version: 2023-06-01` |
| official | `auth.json` exists + parseable; optional spawn `grok models` / `grok version` |

Return: `{ ok, latencyMs, detail }` — detail redacts secrets.

Timeouts: connect 5s, total 20s.

### 6.5 Backup / restore

Before every destructive switch (if `autoBackup`):

```
~/.grok-switch/backups/<ts>/
  config.toml
  auth.json   (if present)
  meta.json   { reason, mode, providerId?, accountId? }
```

Keep last **30** backups (FIFO delete).

Restore: copy files back to grok home; refresh settings current* from meta if present; activity entry.

### 6.6 Rollback on write failure

If config write fails mid-way after backup: restore backup immediately and report error.

---

## 7. UI design

Evolve existing mock in the project root (dark, product-grade).

### Pages

| Page | Purpose |
|------|---------|
| **Overview** | Current mode card, CLI health (path + version), quick switch recent, recent activity |
| **Providers** | List/search, detail pane, Enable / Edit / Delete / Test |
| **Accounts** | Official accounts, Enable / Rename / Delete / Capture |
| **Import** | CC Switch import wizard |
| **Activity** | Full log |
| **Settings** | Paths, official default model, auto backup/health, theme, tray |

### Key UI rules

- Current item: green **Current** pill
- Backend badge: `OpenAI` / `Responses` / `Anthropic`
- Masked API keys; reveal on explicit click
- Empty states with one primary CTA
- Loading overlay on switch (existing pattern)
- Toasts for success/failure
- Chinese UI labels OK (user language); keep code identifiers English

### Tray (Tauri)

- Tooltip: `Grok Switch · {mode}: {name}`
- Menu: recent providers (max 5), recent accounts (max 3), Open, Quit
- Click tray: show main window

### Single instance

Second launch focuses existing window.

---

## 8. Tauri command surface (MVP)

```text
get_settings / update_settings
list_providers / get_provider / upsert_provider / delete_provider
enable_provider(id, { force?: bool })
test_provider(id | inline draft)
list_accounts / upsert_account_meta / delete_account
capture_current_account(name)
enable_account(id)
import_ccswitch_preview()
import_ccswitch_apply(ids[])
get_cli_status() -> { found, path, version?, configOk, authPresent }
list_activity(limit)
list_backups / restore_backup(id)
open_path(kind) // grok home / app data
```

All commands return structured `{ ok, data?, error? }`.

---

## 9. Project layout (target)

```text
grok-switch/
  docs/superpowers/specs/2026-07-13-grok-switch-design.md
  src-tauri/                 # Rust
    src/
      main.rs
      lib.rs
      commands/
      core/
        provider_store.rs
        account_store.rs
        config_writer.rs
        auth_vault.rs
        health.rs
        backup.rs
        ccswitch_import.rs
        paths.rs
  src/                       # React
    main.tsx
    app/
    pages/
    components/
    lib/api.ts
    styles.css
  package.json
  README.md
```

Existing single-file mock (`src/main.tsx`) will be split during implementation; visual language retained.

---

## 10. Implementation phases

Aligned with user delivery preference (runnable UI first, then full product):

### Phase 1 — Frontend showcase (Tauri shell optional / web dev)

- Real page structure for Providers / Accounts / Import / Overview
- Mock data + interaction polish
- Runnable via `npm run dev`

### Phase 2 — Direction confirm

- Pause for user UI confirmation (if needed)

### Phase 3 — Full product backend

- Rust core: config writer, auth vault, import, health, backup
- Wire UI to Tauri commands
- End-to-end switch against real `~/.grok`

### Phase 4 — Delivery

- README: run/build/install
- Example provider templates
- Smoke checklist
- Known limits + next steps (Credential Manager, multi-OS installers, tray polish)

**Note:** Because this product’s value is **real config mutation**, Phase 1 should still show accurate field models; Phase 3 is the critical path. Prefer advancing Phase 3 quickly after UI shell is presentable.

---

## 11. Risks & mitigations

| Risk | Mitigation |
|------|------------|
| TOML clobber of user config | `toml_edit`; only touch `gs-*` + `models.default`; backups |
| Wrong API protocol for relay | Backend selector + health check before enable |
| Per-model key shadows official login | Official mode forces non-`gs-*` default model |
| CC Switch DB schema drift | Defensive parse; preview before apply |
| Concurrent Grok process caching config | Document restart session if needed; Grok hot-reloads auth; config re-read on new session |
| Key leakage in logs | Redaction helpers; never log Authorization headers |

---

## 12. Testing plan

### Automated (as feasible)

- Unit: URL normalize, model name sanitize, TOML rewrite pure functions with fixtures
- Unit: CC Switch env mapping
- Unit: redaction helper

### Manual smoke

1. Fresh `providers.json` → add OpenAI-compatible provider → enable → `grok models` lists `gs-*` → `-p` works  
2. Switch to second provider → only new default active  
3. Capture auth → switch official → default `grok-build`  
4. Import from CC Switch → edit backend → test  
5. Break base URL → health fails → force enable → restore backup  

---

## 13. Decisions locked

| Topic | Decision |
|-------|----------|
| Product scope | Providers **and** official multi-account |
| Form factor | Desktop app |
| Stack | Tauri 2 + React + TS |
| Persistence | JSON under `~/.grok-switch/` |
| Config ownership | Managed sections `model.gs-*` only |
| Import source | `~/.cc-switch/cc-switch.db` Claude providers |
| Env vars | Do **not** rely on injecting process env for Grok; write config files |
| Official login | Capture flow, not embedded OAuth |
| MVP platform | Windows first |

---

## 14. Open items (resolved with defaults)

| Item | Default |
|------|---------|
| Append `/v1` automatically | Yes for new providers; editable |
| Keep inactive `gs-*` when on official | Yes |
| Backup retention | 30 |
| Import backend default | `messages` (Claude Code heritage), user-editable |
| App display name | Grok Switch |
| Package id | `com.grokswitch.app` |

No unresolved blockers for implementation planning.
