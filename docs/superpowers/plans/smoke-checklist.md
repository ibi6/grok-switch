# Grok Switch Smoke Checklist

Date: 2026-07-14

## Build / unit

- [ ] `cd src-tauri && cargo test` — all pass (1 ignored network ok)
- [ ] `npm run build` — tsc + vite pass
- [ ] `npm run tauri:dev` — window opens, tray appears

## Provider path

- [ ] Add OpenAI-compatible provider (e.g. myallapi)
- [ ] Health check ok or force enable
- [ ] `~/.grok/config.toml` contains `[model.gs-...]` and `default = "gs-..."`
- [ ] `grok -p "ok" -m gs-<id>` works

## Import

- [ ] Import page lists CC Switch providers
- [ ] Apply import → appears in Providers
- [ ] Dedup does not double-import same url+key

## Official account

- [ ] Capture current `auth.json`
- [ ] Enable account → `auth.json` replaced, default → `grok-build` (or settings value)
- [ ] Switch back to provider restores gs default

## Backup

- [ ] After switch, `~/.grok-switch/backups/` has new folder with config/auth/meta
- [ ] Restore backup recovers previous files

## Tray / single instance

- [ ] Tray tooltip shows mode/name
- [ ] Tray Open focuses window; Quit exits
- [ ] Second `tauri:dev` / app launch exits with already-running message
