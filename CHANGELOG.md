# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-07-14

### Added

- Desktop app (Tauri 2 + React) for Grok CLI provider / account switching
- Provider CRUD with OpenAI Chat Completions, Responses, and Anthropic Messages backends
- One-click enable writing managed `gs-*` model sections + `endpoints.models_base_url`
- Official account capture / enable via local auth vault
- CC Switch database import (read-only)
- Health probes before enable (optional force)
- Automatic backups and restore UI
- System tray, single-instance focus signal, light/dark/system theme
- Grok-inspired application icon pack

### Security

- Local-only credential storage; masked keys in UI/logs
- Public release scrubbed of personal paths and real secrets

[0.1.0]: https://github.com/ibi6/grok-switch/releases/tag/v0.1.0
