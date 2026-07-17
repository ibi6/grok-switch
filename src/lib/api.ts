import { invoke } from "@tauri-apps/api/core";
import type {
  Account,
  Activity,
  ApiResult,
  BackupInfo,
  CliStatus,
  EnableAccountResult,
  EnableProviderResult,
  HealthResult,
  ImportCandidate,
  Provider,
  ProviderDraft,
  Settings,
} from "./types";

export type { ApiResult } from "./types";

function isTauriRuntime(): boolean {
  return (
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
  );
}

function isInvokeUnavailable(err: unknown): boolean {
  const msg = err instanceof Error ? err.message : String(err ?? "");
  return (
    /not allowed|tauri|invoke|ipc|webview|__TAURI__/i.test(msg) ||
    !isTauriRuntime()
  );
}

// ─── In-memory mock store (npm run dev outside Tauri) ────────────────────────

const now = () => Math.floor(Date.now() / 1000);

const mockSettings: Settings = {
  grokHome: "C:\\Users\\dev\\.grok",
  grokExecutable: "C:\\Users\\dev\\.grok\\bin\\grok.exe",
  currentMode: "provider",
  currentProviderId: "prov-1",
  currentAccountId: "acc-1",
  officialDefaultModel: "grok-build",
  autoBackup: true,
  autoHealthCheck: true,
  launchOnStartup: false,
  theme: "dark",
  trayEnabled: true,
};

let mockProviders: Provider[] = [
  {
    id: "prov-1",
    name: "MyAllAPI",
    baseUrl: "https://api.example.com/v1",
    apiKey: "sk-demo-key-provider-one-xxxx",
    apiBackend: "chat_completions",
    defaultModelEntryId: "m1",
    models: [
      {
        id: "m1",
        model: "grok-4.5",
        name: "Grok 4.5",
        contextWindow: 200_000,
      },
    ],
    contextWindow: 200_000,
    source: "manual",
    createdAt: now() - 86_400,
    updatedAt: now() - 3_600,
  },
  {
    id: "prov-2",
    name: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    apiKey: "sk-or-v1-demo-key-abcdefghijklmnop",
    apiBackend: "chat_completions",
    defaultModelEntryId: "or1",
    models: [
      {
        id: "or1",
        model: "x-ai/grok-4",
        name: "Grok 4",
        contextWindow: 131_072,
      },
    ],
    contextWindow: 131_072,
    websiteUrl: "https://openrouter.ai",
    source: "cc-switch",
    createdAt: now() - 172_800,
    updatedAt: now() - 7_200,
  },
];

let mockAccounts: Account[] = [
  {
    id: "acc-1",
    name: "Studio account",
    email: "studio@northstar.dev",
    labelColor: "#b8f35b",
    status: "ready",
    lastUsedAt: now() - 3_600,
    createdAt: now() - 604_800,
  },
  {
    id: "acc-2",
    name: "Research lab",
    email: "research@northstar.dev",
    labelColor: "#f5b96a",
    status: "ready",
    lastUsedAt: now() - 86_400,
    createdAt: now() - 1_209_600,
  },
];

let mockActivity: Activity[] = [
  {
    ts: now() - 120,
    type: "switch_provider",
    message: "Switched to provider MyAllAPI",
    meta: { providerId: "prov-1", providerName: "MyAllAPI" },
  },
  {
    ts: now() - 3_600,
    type: "health",
    message: "Health check MyAllAPI: ok",
    meta: { providerId: "prov-1", ok: "true" },
  },
  {
    ts: now() - 7_200,
    type: "backup",
    message: "Backup created",
  },
];

let mockBackups: BackupInfo[] = [
  {
    id: "20260713-180000",
    reason: "switch_provider",
    createdAt: now() - 7_200,
    meta: {
      reason: "switch_provider",
      createdAt: now() - 7_200,
      extra: { mode: "provider", providerId: "prov-1" },
    },
  },
];

const mockImportCandidates: ImportCandidate[] = [
  {
    id: "cc-1",
    name: "Claude via Relay",
    baseUrl: "https://relay.example.com",
    apiKey: "sk-import-demo-key-xyz",
    defaultModel: "claude-sonnet-4",
    websiteUrl: "https://example.com",
    suggestedBackend: "messages",
  },
];

function ok<T>(data: T): ApiResult<T> {
  return { ok: true, data };
}

function err<T = never>(message: string): ApiResult<T> {
  return { ok: false, error: message };
}

function pushActivity(
  type: Activity["type"],
  message: string,
  meta?: Record<string, string>,
) {
  mockActivity = [
    { ts: now(), type, message, meta },
    ...mockActivity,
  ].slice(0, 200);
}

async function mockInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<ApiResult<T>> {
  switch (cmd) {
    case "get_settings":
      return ok({ ...mockSettings }) as ApiResult<T>;

    case "update_settings": {
      Object.assign(mockSettings, args?.settings as Settings);
      return ok({ ...mockSettings }) as ApiResult<T>;
    }

    case "list_providers":
      return ok(mockProviders.map((p) => ({ ...p, models: [...p.models] }))) as ApiResult<T>;

    case "upsert_provider": {
      const provider = args?.provider as Provider;
      const idx = mockProviders.findIndex((p) => p.id === provider.id);
      if (idx >= 0) mockProviders[idx] = provider;
      else mockProviders = [...mockProviders, provider];
      return ok(provider) as ApiResult<T>;
    }

    case "delete_provider": {
      const id = String(args?.id ?? "");
      const before = mockProviders.length;
      mockProviders = mockProviders.filter((p) => p.id !== id);
      if (mockSettings.currentProviderId === id) {
        mockSettings.currentProviderId = undefined;
        if (mockSettings.currentMode === "provider") {
          mockSettings.currentMode = "none";
        }
      }
      return ok(before !== mockProviders.length) as ApiResult<T>;
    }

    case "enable_provider": {
      const id = String(args?.id ?? "");
      const force = Boolean(args?.force);
      const provider = mockProviders.find((p) => p.id === id);
      if (!provider) return err(`provider not found: ${id}`) as ApiResult<T>;
      if (mockSettings.autoHealthCheck && !force) {
        // mock always succeeds unless baseUrl is clearly bad
        if (provider.baseUrl.includes("127.0.0.1:1")) {
          return err(
            "NEEDS_FORCE: health check failed: connection refused",
          ) as ApiResult<T>;
        }
      }
      let backupId: string | undefined;
      if (mockSettings.autoBackup) {
        backupId = `mock-${Date.now()}`;
        mockBackups = [
          {
            id: backupId,
            reason: "switch_provider",
            createdAt: now(),
            meta: {
              reason: "switch_provider",
              createdAt: now(),
              extra: { mode: "provider", providerId: id },
            },
          },
          ...mockBackups,
        ];
      }
      mockSettings.currentMode = "provider";
      mockSettings.currentProviderId = id;
      pushActivity("switch_provider", `Switched to provider ${provider.name}`, {
        providerId: id,
        providerName: provider.name,
      });
      const health: HealthResult = {
        ok: true,
        latencyMs: 42,
        detail: "mock ok",
      };
      return ok({
        providerId: id,
        backupId,
        health: mockSettings.autoHealthCheck && !force ? health : undefined,
        postHealth: mockSettings.autoHealthCheck ? health : undefined,
      } satisfies EnableProviderResult) as ApiResult<T>;
    }

    case "test_provider": {
      const id = String(args?.id ?? "");
      const provider = mockProviders.find((p) => p.id === id);
      if (!provider) return err(`provider not found: ${id}`) as ApiResult<T>;
      const result: HealthResult = {
        ok: true,
        latencyMs: 55,
        detail: "mock health ok",
      };
      pushActivity(
        "health",
        `Health check ${provider.name}: ok`,
        { providerId: id, ok: "true" },
      );
      return ok(result) as ApiResult<T>;
    }

    case "test_provider_draft": {
      const draft = args?.draft as ProviderDraft;
      const result: HealthResult = {
        ok: Boolean(draft?.baseUrl && draft?.apiKey),
        latencyMs: 30,
        detail: draft?.baseUrl ? "mock draft ok" : "missing baseUrl",
      };
      return ok(result) as ApiResult<T>;
    }

    case "list_accounts":
      return ok(mockAccounts.map((a) => ({ ...a }))) as ApiResult<T>;

    case "delete_account": {
      const id = String(args?.id ?? "");
      const before = mockAccounts.length;
      mockAccounts = mockAccounts.filter((a) => a.id !== id);
      if (mockSettings.currentAccountId === id) {
        mockSettings.currentAccountId = undefined;
        if (mockSettings.currentMode === "official") {
          mockSettings.currentMode = "none";
        }
      }
      return ok(before !== mockAccounts.length) as ApiResult<T>;
    }

    case "capture_current_account": {
      const name = String(args?.name ?? "Captured");
      const account: Account = {
        id: `acc-${Date.now()}`,
        name,
        email: "captured@local.dev",
        labelColor: "#8cc8ff",
        status: "ready",
        createdAt: now(),
      };
      mockAccounts = [...mockAccounts, account];
      pushActivity("capture_account", `Captured account ${name}`, {
        accountId: account.id,
      });
      return ok(account) as ApiResult<T>;
    }

    case "enable_account": {
      const id = String(args?.id ?? "");
      const account = mockAccounts.find((a) => a.id === id);
      if (!account) return err(`account not found: ${id}`) as ApiResult<T>;
      let backupId: string | undefined;
      if (mockSettings.autoBackup) {
        backupId = `mock-${Date.now()}`;
        mockBackups = [
          {
            id: backupId,
            reason: "switch_account",
            createdAt: now(),
            meta: {
              reason: "switch_account",
              createdAt: now(),
              extra: { mode: "official", accountId: id },
            },
          },
          ...mockBackups,
        ];
      }
      mockAccounts = mockAccounts.map((a) =>
        a.id === id
          ? { ...a, status: "active", lastUsedAt: now() }
          : a.status === "active"
            ? { ...a, status: "ready" }
            : a,
      );
      mockSettings.currentMode = "official";
      mockSettings.currentAccountId = id;
      pushActivity("switch_account", `Switched to account ${account.name}`, {
        accountId: id,
        accountName: account.name,
      });
      return ok({
        accountId: id,
        backupId,
      } satisfies EnableAccountResult) as ApiResult<T>;
    }

    case "import_ccswitch_preview":
      return ok(mockImportCandidates.map((c) => ({ ...c }))) as ApiResult<T>;

    case "import_ccswitch_apply": {
      const ids = (args?.ids as string[]) ?? [];
      const selected = mockImportCandidates.filter((c) => ids.includes(c.id));
      if (selected.length === 0) {
        return err(
          "no matching import candidates for given ids",
        ) as ApiResult<T>;
      }
      const imported: Provider[] = selected.map((c) => {
        const mid = `${c.name}-${c.defaultModel}`
          .toLowerCase()
          .replace(/[^a-z0-9]+/g, "-")
          .replace(/^-+|-+$/g, "")
          .slice(0, 48) || "model";
        return {
          id: `imported-${c.id}`,
          name: c.name,
          baseUrl: c.baseUrl.endsWith("/v1") ? c.baseUrl : `${c.baseUrl.replace(/\/+$/, "")}/v1`,
          apiKey: c.apiKey,
          apiBackend: c.suggestedBackend,
          defaultModelEntryId: mid,
          models: [
            {
              id: mid,
              model: c.defaultModel,
              name: c.defaultModel,
            },
          ],
          contextWindow: 200_000,
          websiteUrl: c.websiteUrl,
          source: "cc-switch",
          createdAt: now(),
          updatedAt: now(),
        };
      });
      mockProviders = [...mockProviders, ...imported];
      pushActivity(
        "import",
        `Imported ${imported.length} provider(s) from CC Switch`,
        { count: String(imported.length) },
      );
      return ok(imported) as ApiResult<T>;
    }

    case "get_cli_status": {
      const status: CliStatus = {
        found: true,
        path: mockSettings.grokExecutable,
        version: "0.8.4",
        configOk: true,
        authPresent: true,
      };
      return ok(status) as ApiResult<T>;
    }

    case "list_activity": {
      const limit = Number(args?.limit ?? 50);
      return ok(mockActivity.slice(0, limit).map((a) => ({ ...a }))) as ApiResult<T>;
    }

    case "list_backups":
      return ok(mockBackups.map((b) => ({ ...b }))) as ApiResult<T>;

    case "restore_backup": {
      const id = String(args?.id ?? "");
      const found = mockBackups.some((b) => b.id === id);
      if (!found) return err(`backup not found: ${id}`) as ApiResult<T>;
      pushActivity("restore", `Restored backup ${id}`, { backupId: id });
      return { ok: true } as ApiResult<T>;
    }

    case "open_grok_terminal": {
      const model = args?.model != null ? String(args.model) : "";
      const token = model.trim();
      if (token && !/^[A-Za-z0-9._/:+-]{1,128}$/.test(token)) {
        return err(
          "model contains invalid characters (allowed: A-Z a-z 0-9 - _ . / : +)",
        ) as ApiResult<T>;
      }
      const cmd = token ? `grok -m ${token}` : "grok";
      return ok(cmd) as ApiResult<T>;
    }

    default:
      return err(`unknown mock command: ${cmd}`) as ApiResult<T>;
  }
}

async function call<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<ApiResult<T>> {
  try {
    return await invoke<ApiResult<T>>(cmd, args);
  } catch (e) {
    if (import.meta.env.DEV && isInvokeUnavailable(e)) {
      return mockInvoke<T>(cmd, args);
    }
    return err(e instanceof Error ? e.message : String(e));
  }
}

// ─── Command wrappers (names match Rust exactly) ─────────────────────────────

export function getSettings() {
  return call<Settings>("get_settings");
}

export function updateSettings(settings: Settings) {
  return call<Settings>("update_settings", { settings });
}

export function listProviders() {
  return call<Provider[]>("list_providers");
}

export function upsertProvider(provider: Provider) {
  return call<Provider>("upsert_provider", { provider });
}

export function deleteProvider(id: string) {
  return call<boolean>("delete_provider", { id });
}

export function enableProvider(id: string, force?: boolean) {
  return call<EnableProviderResult>("enable_provider", { id, force });
}

export function testProvider(id: string) {
  return call<HealthResult>("test_provider", { id });
}

export function testProviderDraft(draft: ProviderDraft) {
  return call<HealthResult>("test_provider_draft", { draft });
}

export function listAccounts() {
  return call<Account[]>("list_accounts");
}

export function deleteAccount(id: string) {
  return call<boolean>("delete_account", { id });
}

export function captureCurrentAccount(name: string) {
  return call<Account>("capture_current_account", { name });
}

export function enableAccount(id: string) {
  return call<EnableAccountResult>("enable_account", { id });
}

export function importCcswitchPreview() {
  return call<ImportCandidate[]>("import_ccswitch_preview");
}

export function importCcswitchApply(ids: string[]) {
  return call<Provider[]>("import_ccswitch_apply", { ids });
}

export function getCliStatus() {
  return call<CliStatus>("get_cli_status");
}

export function listActivity(limit?: number) {
  return call<Activity[]>("list_activity", { limit });
}

export function listBackups() {
  return call<BackupInfo[]>("list_backups");
}

export function restoreBackup(id: string) {
  return call<null>("restore_backup", { id });
}

/** Open a system terminal running `grok` (optionally with `-m <model>`). */
export function openGrokTerminal(model?: string | null) {
  return call<string>("open_grok_terminal", {
    model: model && model.trim() ? model.trim() : null,
  });
}

/** True when running inside a Tauri webview. */
export { isTauriRuntime };
