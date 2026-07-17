# Grok Switch

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows-0078D6?logo=windows)](#)
[![Built with Tauri](https://img.shields.io/badge/Tauri-2.x-24C8DB?logo=tauri)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-19-61DAFB?logo=react)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-1.96+-DEA584?logo=rust)](https://www.rust-lang.org/)

**Grok Switch** is a local desktop control plane for [Grok CLI](https://x.ai/).  
It manages relay providers and official multi-account sessions the way CC Switch does for Claude Code — without sending credentials off-device.

> 中文说明见下方 [中文](#中文)。

---

## Features

| Area | Capability |
|------|------------|
| **Providers** | Create / edit / delete OpenAI-compatible & Anthropic-compatible relays |
| **One-click enable** | Writes managed `gs-*` model sections into `~/.grok/config.toml` |
| **Official accounts** | Capture `grok login` sessions and switch between them |
| **Import** | Read-only import from local CC Switch (`~/.cc-switch/cc-switch.db`) |
| **Skills** | Manage `~/.grok/skills` (`SKILL.md`); import from `~/.cc-switch/skills`; backups on delete |
| **Health checks** | Probe `chat_completions` / `responses` / `messages` before enable |
| **Backups** | Auto-backup before switch; restore from UI |
| **Desktop UX** | Tray icon, single-instance focus, light/dark/system theme |
| **Privacy** | Keys stay under `~/.grok-switch/`; logs mask secrets |

---

## Screenshots

| Providers | Settings |
|-----------|----------|
| Light SaaS-style provider list with enable / test / edit | Theme, paths, auto-backup, auto health-check |

> Run the app locally to explore the full UI (Overview · Providers · Accounts · Import · Skills · Logs & Backups · Settings).

---

## How it works

```text
┌────────────────────┐
│   Grok Switch UI   │  React + Tauri
└─────────┬──────────┘
          │ invoke commands
┌─────────▼──────────┐
│   Rust core        │  providers · auth vault · TOML writer · health · import
└─────────┬──────────┘
          │ filesystem
┌─────────▼──────────┐
│ ~/.grok-switch/    │  app data (providers, settings, accounts, backups)
│ ~/.grok/           │  Grok CLI config.toml + auth.json
│ ~/.cc-switch/      │  optional import source (read-only)
└────────────────────┘
```

Managed model sections use the prefix **`gs-`**:

```toml
[endpoints]
models_base_url = "https://your-relay.example/v1"

[models]
default = "gs-my-relay-grok45"

[model.gs-my-relay-grok45]
model = "grok-4.5"
base_url = "https://your-relay.example/v1"
api_key = "sk-..."
api_backend = "chat_completions"
context_window = 1000000
```

Then:

```powershell
grok -m gs-my-relay-grok45
# or
grok -p "hello" -m gs-my-relay-grok45
```

---

## Requirements

- **Windows 10/11** (primary target)
- **Node.js** 18+
- **Rust** via [rustup](https://rustup.rs/) + MSVC Build Tools
- **Grok CLI** installed (default: `~/.grok/bin/grok.exe`)

---

## Quick start

```powershell
git clone https://github.com/ibi6/grok-switch.git
cd grok-switch
npm install
npm run tauri:dev
```

Frontend-only preview (mock API, does **not** write real Grok config):

```powershell
npm run dev
# http://localhost:1420
```

### Production build

```powershell
npm run tauri:build
```

Artifacts:

- `src-tauri/target/release/grok-switch.exe`
- Installer under `src-tauri/target/release/bundle/`

### Tests

```powershell
cd src-tauri
cargo test
```

---

## Usage

### 1. Add a relay provider

1. Open **Providers** → **Add**
2. Fill name, Base URL (optional auto `/v1`), API key, protocol, default model
3. **Test** → **Enable**
4. Toast shows the managed model id, e.g. `gs-my-relay-grok45` (also clickable on the card)

### 2. Import from CC Switch

1. Open **Import**
2. Preview Claude providers from `~/.cc-switch`
3. Select → import (default backend: Anthropic `messages`, editable later)
4. Enable in **Providers**

### 3. Official multi-account

1. Terminal: `grok login` (or device auth)
2. **Accounts** → **Capture current session**
3. Switch anytime; official mode sets `[models].default` to Settings → official default model (`grok-build` by default)

### 4. Backups

- Auto-backup before switch (toggle in Settings)
- **Logs & Backups** page: list + one-click restore
- Restore overwrites `config.toml` / `auth.json` and clears “current” mode pointers

---

## Data layout

| Path | Purpose |
|------|---------|
| `~/.grok-switch/providers.json` | Provider catalog |
| `~/.grok-switch/settings.json` | App settings |
| `~/.grok-switch/accounts/<id>/` | Official auth snapshots |
| `~/.grok-switch/backups/` | Switch backups (~30 kept) |
| `~/.grok-switch/activity.jsonl` | Local activity log |
| `~/.grok/config.toml` | **Target** — managed `gs-*` models + default |
| `~/.grok/auth.json` | **Target** — official session |
| `~/.cc-switch/cc-switch.db` | Optional import source (read-only) |

---

## API backends

| `api_backend` | Protocol |
|---------------|----------|
| `chat_completions` | OpenAI `/v1/chat/completions` (default) |
| `responses` | OpenAI Responses API |
| `messages` | Anthropic Messages (`x-api-key` via `extra_headers`) |

---

## Security

- Credentials are stored **locally only** (same trust model as typical CLI switchers).
- UI and logs **mask** API keys.
- Never commit `providers.json`, `auth.json`, or backup folders.
- This project is **not** affiliated with xAI / Grok; the icon is a stylized mark for this app.

See [SECURITY.md](./SECURITY.md) for reporting guidance.

---

## Project structure

```text
grok-switch/
├── src/                    # React UI
│   ├── pages/              # Overview, Providers, Accounts, Import, Logs, Settings
│   ├── components/         # Sidebar, forms, toast, overlay
│   └── lib/                # Tauri API bridge + types
├── src-tauri/              # Rust / Tauri
│   └── src/
│       ├── commands/       # invoke surface
│       └── core/           # stores, TOML writer, health, import, backup
├── docs/                   # Design & implementation notes
├── scripts/                # Icon generation helpers
└── README.md
```

---

## Roadmap / limitations

- Windows-first; macOS / Linux packaging not prioritized yet
- No remote sync, billing, or failover queue
- Does **not** mutate global `ANTHROPIC_*` env for Claude Code
- Launch-on-startup is stored but not fully wired to OS autostart

Contributions welcome — see [CONTRIBUTING.md](./CONTRIBUTING.md).

---

## License

[MIT](./LICENSE) © Grok Switch contributors

---

## Disclaimer

Grok Switch is an independent community tool.  
“Grok” and related marks belong to their respective owners.  
Use third-party relays only where you are authorized to do so.

---

## 中文

**Grok Switch** 是面向 Grok CLI 的本地桌面切换器（对标 Claude Code 生态里的 CC Switch）。

### 核心能力

- **中转供应商**：OpenAI / Anthropic 兼容网关，一键写入 `~/.grok/config.toml`
- **官方多账号**：捕获 `grok login` 会话并切换
- **CC Switch 导入**：只读扫描本机 `~/.cc-switch`
- **Skills**：管理 `~/.grok/skills`（`SKILL.md`）；可从 `~/.cc-switch/skills` 导入；删除前备份到 `~/.grok-switch/skill-backups`
- **测通 / 备份 / 托盘 / 浅色深色主题**
- **凭证不出本机**

在 Grok TUI 里用 `/skill-name` 或 `/skills <name>` 调用已安装 skill。

### 快速开始

```powershell
git clone https://github.com/ibi6/grok-switch.git
cd grok-switch
npm install
npm run tauri:dev
```

启用后终端：

```powershell
grok -m gs-<模型条目id>
```

更完整的说明见上文英文章节（数据目录、协议、安全）。
