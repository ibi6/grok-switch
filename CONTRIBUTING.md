# Contributing to Grok Switch

Thanks for your interest in improving Grok Switch.

## Development setup

```powershell
git clone https://github.com/ibi6/grok-switch.git
cd grok-switch
npm install
npm run tauri:dev
```

### Useful commands

| Command | Purpose |
|---------|---------|
| `npm run dev` | Vite UI only (mock backend) |
| `npm run tauri:dev` | Full desktop app |
| `npm run tauri:build` | Release build |
| `cd src-tauri && cargo test` | Rust unit tests |
| `npx tsc --noEmit` | Typecheck frontend |

## Guidelines

1. **No secrets in commits** — never add real API keys, auth dumps, or personal paths.
2. **Keep managed TOML scoped** — only touch `[models].default` and `[model.gs-*]` (+ `endpoints.models_base_url` when enabling relays).
3. **Prefer tests for core logic** — TOML rewrite, normalize/mask, import mapping, enable flows.
4. **Match existing style** — TypeScript React UI, Rust modules under `src-tauri/src/core/`.
5. **Small, focused PRs** — one concern per PR when possible.

## Pull requests

- Describe *what* and *why*.
- Note how you tested (UI path + `cargo test` when Rust changes).
- Update README / docs if user-facing behavior changes.

## Reporting issues

Please include:

- OS version
- Grok CLI version (`grok -v` / `grok version`)
- Steps to reproduce
- Whether the issue is UI-only or config write / CLI routing

Do **not** paste real API keys or full `auth.json` contents.
