# Security Policy

## Local data

Grok Switch stores credentials **only on the local machine**:

- `~/.grok-switch/providers.json` — relay API keys
- `~/.grok-switch/accounts/*/auth.json` — official session snapshots
- `~/.grok/config.toml` / `auth.json` — active Grok CLI state

Treat these paths like password files. Do not share them or commit them to git.

## What we do

- Mask secrets in UI and activity logs
- Avoid writing secrets into git-tracked files
- Prefer filesystem-local operations (no cloud sync)
- Atomic file writes (temp + rename, no delete-before-replace)
- Strict Content-Security-Policy in the Tauri webview
- No shell-plugin surface: terminal launch is a Rust command with model-id whitelist
- Model tokens (CLI `-m` flags, official default model) are validated against `[A-Za-z0-9._/:+-]`

## What we don't claim

- OS keychain / DPAPI encryption is not implemented in the current MVP
- Network health probes send minimal traffic to the configured relay only
- Single-instance lock is best-effort (exclusive file lock under `~/.grok-switch`)

## Reporting a vulnerability

If you discover a security issue:

1. Prefer a **private** report (GitHub Security Advisory if enabled, or contact the repository owner).
2. Do not open a public issue with exploit details until a fix is available.
3. Include reproduction steps and impact assessment.

We appreciate responsible disclosure.
