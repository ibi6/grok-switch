# Grok Switch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Windows-first Tauri desktop app that switches Grok CLI relay providers and official accounts by writing `~/.grok/config.toml` and `~/.grok/auth.json`, with CC Switch import, health checks, and backups.

**Architecture:** React UI talks to a Rust Tauri backend. Core pure modules handle TOML rewrite (`gs-*` model sections), JSON stores under `~/.grok-switch/`, auth vault snapshots, HTTP health probes, and read-only CC Switch DB import. Official Grok CLI remains the runtime; this app only mutates its config/auth files.

**Tech Stack:** Tauri 2, Rust 1.96+, React 18+, TypeScript, Vite, `toml_edit`, `serde`/`serde_json`, `reqwest`, `rusqlite` (bundled), `uuid`, `chrono`.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-13-grok-switch-design.md`
- Managed model sections MUST use prefix `gs-` → tables `[model.gs-<id>]`
- Never delete non-`gs-*` model tables; only mutate `[models].default` + all `[model.gs-*]`
- Secrets stay on-device under `~/.grok-switch/`; logs/UI mask keys (first 6–8 + last 4)
- Do NOT inject `ANTHROPIC_*` / process env for Grok; write config files only
- Official mode sets `[models].default` to `settings.officialDefaultModel` (default `grok-build`)
- Backup before every switch when `autoBackup=true`; keep last 30
- Import source: `~/.cc-switch/cc-switch.db` (read-only)
- Package id: `com.grokswitch.app`; display name: Grok Switch
- Platform: Windows first; paths use user home
- Existing mock UI lives in `src/main.tsx` + `src/styles.css` — evolve, do not throw away visual language
- Prefer small focused files; pure logic unit-tested in Rust before wiring UI

## File map (target)

```text
grok-switch/
  docs/superpowers/specs/2026-07-13-grok-switch-design.md
  docs/superpowers/plans/2026-07-13-grok-switch.md
  package.json
  vite.config.ts
  tsconfig.json
  index.html
  README.md
  src/
    main.tsx
    App.tsx
    styles.css
    lib/
      types.ts
      api.ts
      mask.ts
    pages/
      OverviewPage.tsx
      ProvidersPage.tsx
      AccountsPage.tsx
      ImportPage.tsx
      ActivityPage.tsx
      SettingsPage.tsx
    components/
      Sidebar.tsx
      Toast.tsx
      SwitchOverlay.tsx
      ProviderForm.tsx
      StatusPill.tsx
  src-tauri/
    Cargo.toml
    tauri.conf.json
    capabilities/default.json
    icons/ (generate default)
    src/
      main.rs
      lib.rs
      error.rs
      commands/mod.rs
      core/
        mod.rs
        paths.rs
        types.rs
        mask.rs
        normalize.rs
        provider_store.rs
        account_store.rs
        settings_store.rs
        activity.rs
        backup.rs
        config_writer.rs
        auth_vault.rs
        health.rs
        ccswitch_import.rs
        cli_status.rs
  src-tauri/tests/fixtures/
    sample_config.toml
    sample_ccswitch_env.json
```

---

### Task 1: Scaffold Tauri 2 + React project (keep styles)

**Files:**
- Create/overwrite: `package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`
- Create: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`
- Preserve: `src/styles.css` (existing)
- Move mock temporarily: keep `src/main.tsx` compiling with a stub App

**Interfaces:**
- Produces: `npm run tauri dev` / `npm run dev` runnable shell

- [ ] **Step 1: Init git repo in project root (if missing)**

```powershell
cd grok-switch
git init
```

- [ ] **Step 2: Write package.json scripts and deps**

```json
{
  "name": "grok-switch",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "tauri": "tauri",
    "tauri:dev": "tauri dev",
    "tauri:build": "tauri build"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-dialog": "^2",
    "@tauri-apps/plugin-shell": "^2",
    "lucide-react": "^0.511.0",
    "react": "^19.1.0",
    "react-dom": "^19.1.0"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@types/react": "^19.1.0",
    "@types/react-dom": "^19.1.0",
    "@vitejs/plugin-react": "^4.5.0",
    "typescript": "^5.8.0",
    "vite": "^6.3.0"
  }
}
```

- [ ] **Step 3: Write vite.config.ts for Tauri**

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
```

- [ ] **Step 4: Minimal Tauri Cargo.toml + lib**

`src-tauri/Cargo.toml` must include:

```toml
[package]
name = "grok-switch"
version = "0.1.0"
description = "Grok CLI provider and account switcher"
authors = ["Grok Switch"]
edition = "2021"

[lib]
name = "grok_switch_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-shell = "2"
tauri-plugin-dialog = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml_edit = "0.22"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "blocking"] }
rusqlite = { version = "0.32", features = ["bundled"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2"
dirs = "6"
regex = "1"

[dev-dependencies]
tempfile = "3"
```

`src-tauri/src/lib.rs`:

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running Grok Switch");
}
```

`src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    grok_switch_lib::run();
}
```

- [ ] **Step 5: tauri.conf.json essentials**

- productName: `Grok Switch`
- identifier: `com.grokswitch.app`
- devUrl: `http://localhost:1420`
- frontendDist: `../dist`
- windows: 1180x760, title `Grok Switch`

- [ ] **Step 6: capabilities for FS home access**

Allow core defaults + shell open. File IO will use Rust `std::fs` (no need for broad JS FS scope).

- [ ] **Step 7: npm install && cargo check**

```powershell
cd grok-switch
npm install
cd src-tauri
cargo check
```

Expected: success (may download crates).

- [ ] **Step 8: Commit**

```powershell
cd grok-switch
git add -A
git commit -m "chore: scaffold Tauri 2 + React for Grok Switch"
```

---

### Task 2: Pure helpers — normalize, mask, model sanitize (TDD)

**Files:**
- Create: `src-tauri/src/core/mod.rs`, `normalize.rs`, `mask.rs`, `types.rs`, `error.rs`
- Test: unit tests inside `normalize.rs` / `mask.rs` with `#[cfg(test)]`

**Interfaces:**
- Produces:
  - `normalize_base_url(url: &str, append_v1: bool) -> String`
  - `sanitize_model_name(raw: &str) -> String`
  - `mask_secret(secret: &str) -> String`
  - `gs_model_key(entry_id: &str) -> String` → `gs-{entry_id}`
  - shared types: `ApiBackend`, `Provider`, `ModelEntry`, `Account`, `Settings`, `Activity`, `AppError`

- [ ] **Step 1: Write failing tests for sanitize + normalize**

```rust
#[test]
fn strips_context_suffix() {
    assert_eq!(sanitize_model_name("grok-4.5[1M]"), "grok-4.5");
    assert_eq!(sanitize_model_name("gpt-5.6-sol[1M]"), "gpt-5.6-sol");
    assert_eq!(sanitize_model_name("plain"), "plain");
}

#[test]
fn normalizes_base_url() {
    assert_eq!(
        normalize_base_url("https://relay.example.com:8443/", true),
        "https://relay.example.com:8443/v1"
    );
    assert_eq!(
        normalize_base_url("https://x/v1/", true),
        "https://x/v1"
    );
    assert_eq!(
        normalize_base_url("https://x/v1", false),
        "https://x/v1"
    );
}

#[test]
fn masks_key() {
    let m = mask_secret("sk-demo-key-abcdefghijklmnop");
    assert!(m.starts_with("sk-demo"));
    assert!(m.ends_with("hXR"));
    assert!(m.contains("..."));
    assert!(!m.contains("EN4ckn3c91"));
}
```

- [ ] **Step 2: Run tests — expect fail**

```powershell
cd grok-switch\src-tauri
cargo test sanitize_model_name -- --nocapture
```

Expected: compile fail / test not found.

- [ ] **Step 3: Implement helpers**

`sanitize_model_name`: trim; if matches `^(.+?)\[.+\]$` return capture 1 else original.

`normalize_base_url`: trim; remove trailing `/`; if `append_v1` and not ending with `/v1`, append `/v1`.

`mask_secret`: if len <= 12 return `***`; else `first6 + "..." + last4` (for keys starting with `sk-` keep `sk-` + 4 more if possible).

`gs_model_key(id)`: format `gs-{id}` where `id` is already slug without `gs-` prefix; if id already starts with `gs-`, do not double-prefix.

- [ ] **Step 4: cargo test — expect pass**

```powershell
cargo test -q
```

Expected: all pass.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src
git commit -m "feat: add normalize, mask, and shared core types"
```

---

### Task 3: Settings / provider / account / activity stores

**Files:**
- Create: `paths.rs`, `settings_store.rs`, `provider_store.rs`, `account_store.rs`, `activity.rs`
- Test: tempfile-based store tests

**Interfaces:**
- Produces:
  - `Paths::resolve() -> Paths` with `grok_home`, `app_home`, `config_toml`, `auth_json`, `providers_json`, …
  - `load_settings/save_settings`
  - `list_providers/upsert_provider/delete_provider/get_provider`
  - `list_accounts/save_account_meta/delete_account_dir`
  - `append_activity/list_activity(limit)`

- [ ] **Step 1: Implement Paths**

```rust
pub struct Paths {
    pub home: PathBuf,
    pub app_home: PathBuf,
    pub grok_home: PathBuf,
    pub config_toml: PathBuf,
    pub auth_json: PathBuf,
    pub providers_json: PathBuf,
    pub settings_json: PathBuf,
    pub activity_jsonl: PathBuf,
    pub accounts_dir: PathBuf,
    pub backups_dir: PathBuf,
    pub ccswitch_db: PathBuf,
}
```

Defaults from `dirs::home_dir()`. `ensure_app_dirs()` creates app_home, accounts, backups.

- [ ] **Step 2: Settings defaults**

```rust
Settings {
  grok_home: ~/.grok,
  grok_executable: ~/.grok/bin/grok.exe,
  current_mode: "none",
  official_default_model: "grok-build",
  auto_backup: true,
  auto_health_check: true,
  launch_on_startup: false,
  theme: "dark",
  tray_enabled: true,
}
```

- [ ] **Step 3: Provider store JSON `{ version: 1, items: [] }`**

upsert by id; delete by id; atomic write via temp file + rename.

- [ ] **Step 4: Account store**

`accounts/index.json` + `accounts/<id>/meta.json` + `auth.json`.

- [ ] **Step 5: Activity append JSONL + read last N**

- [ ] **Step 6: Tests with tempfile override**

Inject base dir via `Paths::from_root(tmp)` for tests.

```rust
#[test]
fn provider_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::from_root(dir.path());
    paths.ensure_app_dirs().unwrap();
    // upsert + list + delete
}
```

- [ ] **Step 7: cargo test && commit**

```powershell
cargo test -q
git add src-tauri/src
git commit -m "feat: add JSON stores for settings, providers, accounts, activity"
```

---

### Task 4: config_writer — TOML gs-* rewrite (TDD)

**Files:**
- Create: `src-tauri/src/core/config_writer.rs`
- Create fixture: `src-tauri/tests/fixtures/sample_config.toml`

**Interfaces:**
- Consumes: `Provider`, `ModelEntry`, `ApiBackend`
- Produces:
  - `apply_provider(config_text: &str, provider: &Provider) -> Result<String>`
  - `apply_official_default(config_text: &str, model: &str) -> Result<String>`
  - `read_config(path) / write_config(path, text)`

- [ ] **Step 1: Fixture with user custom model + old gs block**

```toml
[cli]
installer = "internal"

[ui]
yolo = false

[models]
default = "gs-old-model"

[model.gs-old-model]
model = "old"
base_url = "https://old/v1"
api_key = "sk-old"

[model.user-custom]
model = "keep-me"
base_url = "https://custom/v1"
```

- [ ] **Step 2: Failing test — apply provider removes old gs, keeps user-custom, sets default**

```rust
#[test]
fn apply_provider_rewrites_only_gs() {
    let input = include_str!("../../tests/fixtures/sample_config.toml");
    let provider = /* Provider with one model entry id myallapi-grok45 */;
    let out = apply_provider(input, &provider).unwrap();
    assert!(out.contains("[model.gs-myallapi-grok45]"));
    assert!(!out.contains("[model.gs-old-model]"));
    assert!(out.contains("[model.user-custom]"));
    assert!(out.contains("default = \"gs-myallapi-grok45\"") || out.contains("default = 'gs-myallapi-grok45'"));
    assert!(out.contains("keep-me"));
}
```

- [ ] **Step 3: Implement with toml_edit::DocumentMut**

Algorithm:
1. Parse document
2. Ensure `[models]` table exists
3. Collect keys under root starting with `model.gs-` OR document as nested — **Grok uses dotted tables** `[model.NAME]` which toml_edit represents as `model` table with key `NAME`. Prefer iterating `doc.as_table_mut()` for keys matching pattern; also handle inline `doc["model"]["gs-..."]`.
4. Remove every key in `doc["model"]` that starts with `gs-`
5. For each `ModelEntry`, create table with fields per backend
6. Set `doc["models"]["default"] = gs_model_key(provider.default_model_entry_id)`
7. Return `doc.to_string()`

For `messages` backend set `extra_headers` as inline table; omit `api_key`.

- [ ] **Step 4: Test official default**

```rust
#[test]
fn apply_official_sets_default_keeps_gs() {
    let out = apply_official_default(input, "grok-build").unwrap();
    assert!(out.contains("default = \"grok-build\""));
    assert!(out.contains("[model.gs-")); // may keep after provider apply first
}
```

- [ ] **Step 5: cargo test && commit**

```powershell
cargo test config_writer -q
git add src-tauri
git commit -m "feat: TOML config writer for gs-* model sections"
```

---

### Task 5: backup + auth_vault

**Files:**
- Create: `backup.rs`, `auth_vault.rs`

**Interfaces:**
- Produces:
  - `create_backup(paths, reason, meta) -> backup_id`
  - `restore_backup(paths, backup_id)`
  - `prune_backups(paths, keep=30)`
  - `capture_auth(paths, account_id, name) -> Account`
  - `enable_auth(paths, account_id)` copies vault → grok auth.json

- [ ] **Step 1: Backup copies config.toml + auth.json if exist into timestamp dir + meta.json**

- [ ] **Step 2: restore overwrites grok files from backup**

- [ ] **Step 3: capture requires auth.json exists else error**

- [ ] **Step 4: Tests with temp paths**

- [ ] **Step 5: cargo test && commit**

```powershell
git commit -m "feat: backup and official auth vault"
```

---

### Task 6: health checks

**Files:**
- Create: `health.rs`

**Interfaces:**
- Produces:
  - `check_provider(base_url, api_key, backend, model) -> HealthResult { ok, latency_ms, detail }`
  - `check_official(paths) -> HealthResult`

- [ ] **Step 1: Implement blocking reqwest client with 5s connect / 20s total**

- [ ] **Step 2: chat_completions: GET `{base}/models` Authorization Bearer; on non-success POST chat/completions minimal**

Minimal body:

```json
{"model":"<model>","messages":[{"role":"user","content":"ping"}],"max_tokens":1}
```

- [ ] **Step 3: messages: POST `{base}/messages` with headers x-api-key, anthropic-version: 2023-06-01, content-type application/json**

```json
{"model":"<model>","max_tokens":1,"messages":[{"role":"user","content":"ping"}]}
```

- [ ] **Step 4: responses: POST `{base}/responses` minimal**

- [ ] **Step 5: Redact any accidental key in detail string via mask_secret**

- [ ] **Step 6: Unit test redaction; optional `#[ignore]` network test**

- [ ] **Step 7: commit**

```powershell
git commit -m "feat: provider and official health probes"
```

---

### Task 7: CC Switch import

**Files:**
- Create: `ccswitch_import.rs`

**Interfaces:**
- Produces:
  - `preview_ccswitch(db_path) -> Vec<ImportCandidate>`
  - `candidates_to_providers(selected) -> Vec<Provider>`

```rust
pub struct ImportCandidate {
  pub id: String,          // original cc-switch id
  pub name: String,
  pub base_url: String,
  pub api_key: String,
  pub default_model: String,
  pub website_url: Option<String>,
  pub suggested_backend: ApiBackend, // default Messages
}
```

- [ ] **Step 1: Open SQLite read-only**

```rust
let uri = format!("file:{}?mode=ro", db_path.display());
Connection::open_with_flags(uri, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI)
```

- [ ] **Step 2: Query**

```sql
SELECT id, name, settings_config, website_url FROM providers
WHERE app_type = 'claude'
```

- [ ] **Step 3: Parse settings_config JSON → env map; require ANTHROPIC_BASE_URL + ANTHROPIC_AUTH_TOKEN**

Model from `ANTHROPIC_DEFAULT_SONNET_MODEL` else `ANTHROPIC_MODEL` else `grok-build`; run `sanitize_model_name`.

- [ ] **Step 4: Build Provider**

- `source: CcSwitch`
- `api_backend: Messages` (default)
- one `ModelEntry` with id slug from `{name}-{model}` sanitized to `[a-z0-9-]+`
- `default_model_entry_id` that entry
- `base_url` via `normalize_base_url(..., true)`

- [ ] **Step 5: Dedup against existing providers (normalized url + key equality)**

- [ ] **Step 6: Test with in-memory sqlite fixture inserting one row**

- [ ] **Step 7: commit**

```powershell
git commit -m "feat: import providers from CC Switch database"
```

---

### Task 8: Orchestration services + Tauri commands

**Files:**
- Create: `src-tauri/src/commands/mod.rs`, wire in `lib.rs`
- Create: `cli_status.rs`

**Interfaces:**
- Produces invoke commands matching design §8:

```text
get_settings, update_settings
list_providers, upsert_provider, delete_provider
enable_provider(id, force?)
test_provider / test_provider_draft
list_accounts, delete_account, capture_current_account, enable_account
import_ccswitch_preview, import_ccswitch_apply
get_cli_status
list_activity
list_backups, restore_backup
```

- [ ] **Step 1: Define `ApiResult<T> { ok: bool, data: Option<T>, error: Option<String> }`**

- [ ] **Step 2: enable_provider flow**

```
load provider
if auto_health && !force: check; on fail return error code NEEDS_FORCE
if auto_backup: create_backup
read config → apply_provider → write config
settings.current_mode=provider; current_provider_id=id
activity switch_provider
if auto_health: post-check (soft fail)
return ok
```

- [ ] **Step 3: enable_account flow**

backup → copy auth → apply_official_default on config → settings official → activity

- [ ] **Step 4: get_cli_status**

- path exists?
- run `grok -v` or `grok version` capturing stdout (timeout 5s)
- config parse ok?
- auth present?

- [ ] **Step 5: Register all commands in `invoke_handler!`**

- [ ] **Step 6: Manual smoke from `cargo test` for enable_provider on temp Paths (inject Paths via thread-local or pass AppHandle state `AppState { paths }`)**

Use `tauri::State<AppState>`.

- [ ] **Step 7: commit**

```powershell
git commit -m "feat: Tauri commands for switch, import, health, backup"
```

---

### Task 9: Frontend types + api bridge + mask util

**Files:**
- Create: `src/lib/types.ts`, `src/lib/api.ts`, `src/lib/mask.ts`

**Interfaces:**
- Produces TS types mirroring Rust serde names (snake_case in JSON — configure serde `rename_all = "camelCase"` on Rust structs for nicer TS **OR** use snake_case in TS; **lock: use camelCase via serde rename_all = "camelCase"` on all exported DTOs**)

- [ ] **Step 1: Add `#[serde(rename_all = "camelCase")]` on all command DTOs if not already**

- [ ] **Step 2: api.ts**

```ts
import { invoke } from "@tauri-apps/api/core";

export type ApiResult<T> = { ok: boolean; data?: T; error?: string };

export async function listProviders() {
  return invoke<ApiResult<Provider[]>>("list_providers");
}
// ... one function per command
```

- [ ] **Step 3: Dev mock fallback**

If `import.meta.env.DEV` and invoke throws `Tauri` not available, serve in-memory mock data so `npm run dev` still demos UI.

- [ ] **Step 4: commit**

```powershell
git commit -m "feat: frontend API bridge and shared types"
```

---

### Task 10: UI pages — real product shell

**Files:**
- Create: `App.tsx`, pages/*, components/*
- Modify: `main.tsx`, `styles.css` (extend, keep aesthetic)

**Interfaces:**
- Consumes: `src/lib/api.ts`
- Produces: navigable UI for all pages

- [ ] **Step 1: App shell with sidebar routes: Overview, Providers, Accounts, Import, Activity, Settings**

- [ ] **Step 2: Overview — current mode card, CLI status, recent activity, CTA buttons**

- [ ] **Step 3: Providers — list + detail + Enable/Test/Edit/Delete + add modal form (name, baseUrl, apiKey, backend select, default model, appendV1 checkbox)**

- [ ] **Step 4: Accounts — list + Capture + Enable + Delete**

- [ ] **Step 5: Import — preview checklist from import_ccswitch_preview → apply**

- [ ] **Step 6: Activity + Settings forms wired to API**

- [ ] **Step 7: Toast + switching overlay (reuse existing UX patterns)**

- [ ] **Step 8: `npm run dev` visual check — empty states, forms, no broken layout**

- [ ] **Step 9: commit**

```powershell
git commit -m "feat: product UI for providers, accounts, import, settings"
```

---

### Task 11: Tray + single instance + polish

**Files:**
- Modify: `src-tauri/src/lib.rs`, `tauri.conf.json`

- [ ] **Step 1: System tray with tooltip `Grok Switch · {mode}: {name}`**

- [ ] **Step 2: Menu: Open, Quit; optional quick switch later if time**

- [ ] **Step 3: Single instance plugin or manual mutex — focus existing window**

Prefer `tauri-plugin-single-instance` if stable on Tauri 2; else document limitation.

- [ ] **Step 4: commit**

```powershell
git commit -m "feat: tray icon and single-instance behavior"
```

---

### Task 12: End-to-end smoke + README delivery

**Files:**
- Create: `README.md`
- Create: `docs/superpowers/plans/smoke-checklist.md` (optional short)

- [ ] **Step 1: README with**

- 启动: `npm install` → `npm run tauri:dev`
- 构建: `npm run tauri:build`
- 数据目录: `~/.grok-switch`
- 写入目标: `~/.grok/config.toml`, `auth.json`
- 从 CC Switch 导入步骤
- 官方账号捕获步骤
- 功能清单 / 已知限制

- [ ] **Step 2: Manual E2E against real machine**

1. Add provider pointing at current myallapi URL (user pastes key)  
2. Test health  
3. Enable → open terminal:  
   `grok models`  
   `grok -p "只回复 ok" -m gs-<id>`  
4. Import from CC Switch  
5. Capture auth (if logged in) → switch official → switch back  
6. Restore a backup  

- [ ] **Step 3: Fix any bugs found**

- [ ] **Step 4: Final commit**

```powershell
git commit -m "docs: README and smoke verification for Grok Switch MVP"
```

---

## Spec coverage checklist

| Spec requirement | Task |
|------------------|------|
| Provider CRUD + enable | 3, 4, 8, 10 |
| Official multi-account | 5, 8, 10 |
| Health check OpenAI + messages | 6, 8 |
| CC Switch import | 7, 8, 10 |
| Backups + restore | 5, 8, 10 |
| gs-* TOML contract | 4 |
| Mask secrets | 2, 10 |
| Tray + paths + settings | 3, 8, 11 |
| UI pages | 10 |
| Delivery README | 12 |

## Execution notes for agents

- Work only under the `grok-switch` project root unless installing global tools.
- After Task 4, prefer integration tests before large UI work.
- Never print full API keys in commit messages, logs, or README examples — use placeholders `sk-...`.
- If Tauri icon generation needed: `npm run tauri icon` with a simple PNG, or use default.
- User delivery preference: keep UI runnable early (Task 1 + 10 mock path) but prioritize real switch correctness (Tasks 2–8).

---

## Plan self-review

- No TBD placeholders remain.
- Serde JSON casing locked to **camelCase** for frontend.
- `enable_provider` force flag matches design “仍要启用”.
- Import default backend **messages** matches design §6.3.
- Backup retention 30 matches design.
