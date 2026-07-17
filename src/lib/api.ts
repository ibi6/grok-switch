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
  SkillDetail,
  SkillDraft,
  SkillInfo,
  McpDraft,
  McpHealthResult,
  McpServer,
  ProxyStatus,
  PromptRow,
  RequestLog,
  TokenStats,
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
  proxyEnabled: false,
  proxyPort: 18765,
  poolStrategy: "priority",
  silentStartup: false,
  preferredTerminal: "windows_terminal",
  autoSkillSync: false,
  confirmOnSwitch: false,
};

let mockProxy: ProxyStatus = { running: false, port: 18765, listen: "" };
let mockRequestLogs: RequestLog[] = [];
let mockPrompts: PromptRow[] = [
  {
    id: "prompt-1",
    name: "代码审查",
    body: "请审查以下改动，关注正确性、安全与可维护性。",
    updatedAt: now() - 3600,
  },
];

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

let mockSkills: SkillInfo[] = [
  {
    name: "check-work",
    description: "Verify changes with a subagent",
    path: "C:\\Users\\dev\\.grok\\skills\\check-work",
    skillMdPath: "C:\\Users\\dev\\.grok\\skills\\check-work\\SKILL.md",
    scope: "grok",
    isSymlink: false,
    hasSkillMd: true,
    editable: true,
  },
  {
    name: "brainstorming",
    description: "Explore requirements before implementation",
    path: "C:\\Users\\dev\\.grok\\skills\\brainstorming",
    skillMdPath: "C:\\Users\\dev\\.grok\\skills\\brainstorming\\SKILL.md",
    scope: "grok",
    isSymlink: true,
    linkTarget: "C:\\Users\\dev\\.codeg\\skills\\brainstorming",
    hasSkillMd: true,
    editable: false,
  },
];

const mockSkillContents: Record<string, string> = {
  "check-work":
    "---\nname: check-work\ndescription: Verify changes with a subagent\n---\n\n# Check work\n\nRun verification steps.\n",
  brainstorming:
    "---\nname: brainstorming\ndescription: Explore requirements before implementation\n---\n\n# Brainstorm\n",
};

let mockMcpServers: McpServer[] = [
  {
    name: "filesystem",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-filesystem", "C:\\Users\\dev"],
    env: {},
    headers: {},
    enabled: true,
    startupTimeoutSec: 30,
    transport: "stdio",
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
      const incoming = args?.settings as Settings;
      // Mirror Rust merge_user_settings: preserve runtime current_* pointers.
      mockSettings.grokHome = incoming.grokHome;
      mockSettings.grokExecutable = incoming.grokExecutable;
      mockSettings.officialDefaultModel = incoming.officialDefaultModel;
      mockSettings.autoBackup = incoming.autoBackup;
      mockSettings.autoHealthCheck = incoming.autoHealthCheck;
      mockSettings.launchOnStartup = incoming.launchOnStartup;
      mockSettings.theme = incoming.theme;
      mockSettings.trayEnabled = incoming.trayEnabled;
      mockSettings.proxyEnabled = incoming.proxyEnabled;
      mockSettings.proxyPort = incoming.proxyPort;
      mockSettings.poolStrategy = incoming.poolStrategy;
      mockSettings.silentStartup = incoming.silentStartup;
      mockSettings.preferredTerminal = incoming.preferredTerminal;
      mockSettings.autoSkillSync = incoming.autoSkillSync;
      mockSettings.confirmOnSwitch = incoming.confirmOnSwitch;
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

    case "duplicate_provider": {
      const id = String(args?.id ?? "");
      const src = mockProviders.find((p) => p.id === id);
      if (!src) return err(`provider not found: ${id}`) as ApiResult<T>;
      const copy: Provider = {
        ...src,
        id: `prov-${Date.now()}`,
        name: `${src.name} (副本)`,
        createdAt: now(),
        updatedAt: now(),
        cooldownUntil: undefined,
        source: "manual",
      };
      mockProviders = [...mockProviders, copy];
      return ok(copy) as ApiResult<T>;
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

    case "upsert_account": {
      const account = args?.account as Account;
      if (!account?.id) return err("account id required") as ApiResult<T>;
      const idx = mockAccounts.findIndex((a) => a.id === account.id);
      if (idx < 0) return err(`account not found: ${account.id}`) as ApiResult<T>;
      mockAccounts[idx] = { ...mockAccounts[idx], ...account };
      return ok({ ...mockAccounts[idx] }) as ApiResult<T>;
    }

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

    case "list_skills":
      return ok(mockSkills.map((s) => ({ ...s }))) as ApiResult<T>;

    case "get_skill": {
      const name = String(args?.name ?? "");
      const info = mockSkills.find((s) => s.name === name);
      if (!info) return err(`skill not found: ${name}`) as ApiResult<T>;
      return ok({
        info: { ...info },
        content:
          mockSkillContents[name] ??
          `---\nname: ${name}\ndescription: ${info.description}\n---\n\n# ${name}\n`,
      } satisfies SkillDetail) as ApiResult<T>;
    }

    case "upsert_skill": {
      const draft = args?.draft as SkillDraft;
      if (!draft?.name) return err("skill name required") as ApiResult<T>;
      if (!/^[a-z0-9]([a-z0-9-]*[a-z0-9])?$/.test(draft.name) || draft.name.length < 2) {
        return err("invalid skill name") as ApiResult<T>;
      }
      const content = `---\nname: ${draft.name}\ndescription: ${draft.description}\n---\n\n${draft.content.replace(/^---[\s\S]*?---\s*/, "")}\n`;
      mockSkillContents[draft.name] = content;
      const info: SkillInfo = {
        name: draft.name,
        description: draft.description,
        path: `C:\\Users\\dev\\.grok\\skills\\${draft.name}`,
        skillMdPath: `C:\\Users\\dev\\.grok\\skills\\${draft.name}\\SKILL.md`,
        scope: "grok",
        isSymlink: false,
        hasSkillMd: true,
        editable: true,
      };
      const idx = mockSkills.findIndex((s) => s.name === draft.name);
      if (idx >= 0) mockSkills[idx] = info;
      else mockSkills = [...mockSkills, info];
      return ok({ info, content } satisfies SkillDetail) as ApiResult<T>;
    }

    case "delete_skill": {
      const name = String(args?.name ?? "");
      const before = mockSkills.length;
      mockSkills = mockSkills.filter((s) => s.name !== name);
      delete mockSkillContents[name];
      return ok(before !== mockSkills.length) as ApiResult<T>;
    }

    case "import_skills": {
      const imported: SkillInfo[] = [
        {
          name: "imported-demo",
          description: "Imported from CC Switch (mock)",
          path: "C:\\Users\\dev\\.grok\\skills\\imported-demo",
          skillMdPath: "C:\\Users\\dev\\.grok\\skills\\imported-demo\\SKILL.md",
          scope: "grok",
          isSymlink: false,
          hasSkillMd: true,
          editable: true,
        },
      ];
      for (const s of imported) {
        if (!mockSkills.some((x) => x.name === s.name)) mockSkills.push(s);
      }
      return ok(imported) as ApiResult<T>;
    }

    case "list_mcp_servers":
      return ok(mockMcpServers.map((s) => ({ ...s, args: [...s.args], env: { ...s.env }, headers: { ...s.headers } }))) as ApiResult<T>;

    case "get_mcp_server": {
      const name = String(args?.name ?? "");
      const s = mockMcpServers.find((x) => x.name === name);
      if (!s) return err(`MCP server not found: ${name}`) as ApiResult<T>;
      return ok({ ...s, args: [...s.args], env: { ...s.env }, headers: { ...s.headers } }) as ApiResult<T>;
    }

    case "upsert_mcp_server": {
      const draft = args?.draft as McpDraft;
      if (!draft?.name) return err("name required") as ApiResult<T>;
      if (!/^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/.test(draft.name) || draft.name.endsWith("-")) {
        return err("invalid MCP name") as ApiResult<T>;
      }
      const hasCmd = Boolean(draft.command?.trim());
      const hasUrl = Boolean(draft.url?.trim());
      if (!hasCmd && !hasUrl) return err("need command or url") as ApiResult<T>;
      const server: McpServer = {
        name: draft.name,
        command: draft.command,
        args: draft.args ?? [],
        url: draft.url,
        env: draft.env ?? {},
        headers: draft.headers ?? {},
        enabled: draft.enabled ?? true,
        startupTimeoutSec: draft.startupTimeoutSec,
        toolTimeoutSec: draft.toolTimeoutSec,
        transport: hasUrl ? "http" : hasCmd ? "stdio" : "unknown",
      };
      const idx = mockMcpServers.findIndex((x) => x.name === server.name);
      if (idx >= 0) mockMcpServers[idx] = server;
      else mockMcpServers = [...mockMcpServers, server];
      return ok(server) as ApiResult<T>;
    }

    case "delete_mcp_server": {
      const name = String(args?.name ?? "");
      const before = mockMcpServers.length;
      mockMcpServers = mockMcpServers.filter((s) => s.name !== name);
      return ok(before !== mockMcpServers.length) as ApiResult<T>;
    }

    case "set_mcp_enabled": {
      const name = String(args?.name ?? "");
      const enabled = Boolean(args?.enabled);
      const idx = mockMcpServers.findIndex((s) => s.name === name);
      if (idx < 0) return err(`MCP server not found: ${name}`) as ApiResult<T>;
      mockMcpServers[idx] = { ...mockMcpServers[idx], enabled };
      return ok({ ...mockMcpServers[idx] }) as ApiResult<T>;
    }

    case "test_mcp_server": {
      const name = String(args?.name ?? "");
      const s = mockMcpServers.find((x) => x.name === name);
      if (!s) return err(`MCP server not found: ${name}`) as ApiResult<T>;
      const result: McpHealthResult = {
        ok: true,
        detail: s.transport === "http" ? "HTTP 200 (mock)" : `command on PATH: ${s.command ?? "?"}`,
        latencyMs: 12,
      };
      return ok(result) as ApiResult<T>;
    }

    case "list_request_logs": {
      const limit = Number(args?.limit ?? 100);
      return ok(mockRequestLogs.slice(0, limit)) as ApiResult<T>;
    }

    case "get_token_stats": {
      const stats: TokenStats = mockRequestLogs.reduce(
        (acc, l) => {
          acc.requests += 1;
          acc.promptTokens += l.promptTokens;
          acc.completionTokens += l.completionTokens;
          if (l.ok) acc.okCount += 1;
          else acc.failCount += 1;
          return acc;
        },
        {
          requests: 0,
          promptTokens: 0,
          completionTokens: 0,
          okCount: 0,
          failCount: 0,
        } satisfies TokenStats,
      );
      return ok(stats) as ApiResult<T>;
    }

    case "get_proxy_status":
      return ok({ ...mockProxy }) as ApiResult<T>;

    case "start_proxy": {
      mockProxy = {
        running: true,
        port: mockSettings.proxyPort ?? 18765,
        listen: `http://127.0.0.1:${mockSettings.proxyPort ?? 18765}/v1`,
      };
      mockSettings.proxyEnabled = true;
      return ok({ ...mockProxy }) as ApiResult<T>;
    }

    case "stop_proxy": {
      mockProxy = { running: false, port: mockSettings.proxyPort ?? 18765, listen: "" };
      mockSettings.proxyEnabled = false;
      return ok({ ...mockProxy }) as ApiResult<T>;
    }

    case "clear_provider_cooldown": {
      const id = String(args?.id ?? "");
      const idx = mockProviders.findIndex((p) => p.id === id);
      if (idx < 0) return err(`provider not found: ${id}`) as ApiResult<T>;
      mockProviders[idx] = { ...mockProviders[idx], cooldownUntil: undefined };
      return ok({ ...mockProviders[idx] }) as ApiResult<T>;
    }

    case "list_prompts":
      return ok([...mockPrompts]) as ApiResult<T>;

    case "upsert_prompt": {
      const id = String(args?.id ?? `prompt-${Date.now()}`);
      const name = String(args?.name ?? "untitled");
      const body = String(args?.body ?? "");
      const row: PromptRow = {
        id,
        name,
        body,
        updatedAt: now(),
      };
      const idx = mockPrompts.findIndex((p) => p.id === id);
      if (idx >= 0) mockPrompts[idx] = row;
      else mockPrompts = [row, ...mockPrompts];
      return ok(row) as ApiResult<T>;
    }

    case "delete_prompt": {
      const id = String(args?.id ?? "");
      const before = mockPrompts.length;
      mockPrompts = mockPrompts.filter((p) => p.id !== id);
      return ok(before !== mockPrompts.length) as ApiResult<T>;
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

export function duplicateProvider(id: string) {
  return call<Provider>("duplicate_provider", { id });
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

export function upsertAccount(account: Account) {
  return call<Account>("upsert_account", { account });
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

export function listSkills() {
  return call<SkillInfo[]>("list_skills");
}

export function getSkill(name: string) {
  return call<SkillDetail>("get_skill", { name });
}

export function upsertSkill(draft: SkillDraft) {
  return call<SkillDetail>("upsert_skill", { draft });
}

export function deleteSkill(name: string) {
  return call<boolean>("delete_skill", { name });
}

export function importSkills(names: string[] = [], source: "cc-switch" | "claude" = "cc-switch") {
  return call<SkillInfo[]>("import_skills", { names, source });
}

export function listMcpServers() {
  return call<McpServer[]>("list_mcp_servers");
}

export function getMcpServer(name: string) {
  return call<McpServer>("get_mcp_server", { name });
}

export function upsertMcpServer(draft: McpDraft) {
  return call<McpServer>("upsert_mcp_server", { draft });
}

export function deleteMcpServer(name: string) {
  return call<boolean>("delete_mcp_server", { name });
}

export function setMcpEnabled(name: string, enabled: boolean) {
  return call<McpServer>("set_mcp_enabled", { name, enabled });
}

export function testMcpServer(name: string) {
  return call<McpHealthResult>("test_mcp_server", { name });
}

export function listRequestLogs(limit?: number) {
  return call<RequestLog[]>("list_request_logs", { limit });
}

export function getTokenStats() {
  return call<TokenStats>("get_token_stats");
}

export function getProxyStatus() {
  return call<ProxyStatus>("get_proxy_status");
}

export function startProxy() {
  return call<ProxyStatus>("start_proxy");
}

export function stopProxy() {
  return call<ProxyStatus>("stop_proxy");
}

export function clearProviderCooldown(id: string) {
  return call<Provider>("clear_provider_cooldown", { id });
}

export function listPrompts() {
  return call<PromptRow[]>("list_prompts");
}

export function upsertPrompt(id: string, name: string, body: string) {
  return call<PromptRow>("upsert_prompt", { id, name, body });
}

export function deletePrompt(id: string) {
  return call<boolean>("delete_prompt", { id });
}

/** True when running inside a Tauri webview. */
export { isTauriRuntime };
