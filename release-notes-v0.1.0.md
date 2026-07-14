## Grok Switch v0.1.0

First public release of **Grok Switch** — a local desktop control plane for Grok CLI.

### Highlights

- Provider CRUD for OpenAI-compatible & Anthropic-compatible relays
- One-click enable → managed `gs-*` models in `~/.grok/config.toml`
- Official multi-account capture / switch
- Import from local CC Switch database
- Health checks, auto-backup + restore UI
- Tray, single-instance focus, light/dark/system theme

### Install (from source)

```powershell
git clone https://github.com/ibi6/grok-switch.git
cd grok-switch
npm install
npm run tauri:dev
```

### Notes

- Windows-first
- Credentials stay on-device under `~/.grok-switch/`
- Not affiliated with xAI

See [CHANGELOG.md](https://github.com/ibi6/grok-switch/blob/main/CHANGELOG.md) for details.
