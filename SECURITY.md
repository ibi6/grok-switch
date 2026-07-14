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

## What we don't claim

- OS keychain / DPAPI encryption is not implemented in the current MVP
- Network health probes send minimal traffic to the configured relay only

## Reporting a vulnerability

If you discover a security issue:

1. Prefer a **private** report (GitHub Security Advisory if enabled, or contact the repository owner).
2. Do not open a public issue with exploit details until a fix is available.
3. Include reproduction steps and impact assessment.

We appreciate responsible disclosure.
