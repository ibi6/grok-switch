/** Frontend mirrors of Rust serde DTOs (camelCase fields; enum values as noted). */

// Enums use Rust serde rename_all values.

/** ApiBackend: snake_case */
export type ApiBackend = "chat_completions" | "responses" | "messages";

/** ProviderSource: kebab-case */
export type ProviderSource = "manual" | "cc-switch";

/** AccountStatus: snake_case */
export type AccountStatus = "ready" | "active" | "expired" | "unknown";

/** AppMode: snake_case */
export type AppMode = "provider" | "official" | "none";

/** Theme: snake_case */
export type Theme = "system" | "dark" | "light";

/** ActivityType: snake_case */
export type ActivityType =
  | "switch_provider"
  | "switch_account"
  | "import"
  | "health"
  | "backup"
  | "restore"
  | "error"
  | "capture_account"
  | "skill"
  | "mcp"
  | "proxy"
  | "failover";

export interface ModelEntry {
  /** Section key WITHOUT `gs-` prefix */
  id: string;
  /** API model id */
  model: string;
  /** UI label */
  name: string;
  contextWindow?: number;
  apiBackend?: ApiBackend;
}

export interface Provider {
  id: string;
  name: string;
  baseUrl: string;
  apiKey: string;
  apiBackend: ApiBackend;
  defaultModelEntryId: string;
  models: ModelEntry[];
  extraHeaders?: Record<string, string>;
  contextWindow: number;
  websiteUrl?: string;
  notes?: string;
  source: ProviderSource;
  createdAt: number;
  updatedAt: number;
  priority?: number;
  weight?: number;
  poolEnabled?: boolean;
  cooldownUntil?: number;
}

export interface Account {
  id: string;
  name: string;
  email?: string;
  labelColor: string;
  status: AccountStatus;
  lastUsedAt?: number;
  createdAt: number;
  priority?: number;
  weight?: number;
  poolEnabled?: boolean;
  cooldownUntil?: number;
}

export type PoolStrategy = "priority" | "weighted" | "round_robin";
export type PreferredTerminal = "windows_terminal" | "powershell" | "cmd";

export interface Settings {
  grokHome: string;
  grokExecutable: string;
  currentMode: AppMode;
  currentProviderId?: string;
  currentAccountId?: string;
  officialDefaultModel: string;
  autoBackup: boolean;
  autoHealthCheck: boolean;
  launchOnStartup: boolean;
  theme: Theme;
  trayEnabled: boolean;
  proxyEnabled?: boolean;
  proxyPort?: number;
  poolStrategy?: PoolStrategy;
  silentStartup?: boolean;
  preferredTerminal?: PreferredTerminal;
  autoSkillSync?: boolean;
  confirmOnSwitch?: boolean;
}

export interface RequestLog {
  id: number;
  ts: number;
  providerId?: string;
  model?: string;
  method: string;
  path: string;
  status: number;
  latencyMs: number;
  promptTokens: number;
  completionTokens: number;
  ok: boolean;
  detail: string;
}

export interface TokenStats {
  requests: number;
  promptTokens: number;
  completionTokens: number;
  okCount: number;
  failCount: number;
}

export interface ProxyStatus {
  running: boolean;
  port: number;
  listen: string;
}

export interface PromptRow {
  id: string;
  name: string;
  body: string;
  updatedAt: number;
}

export interface Activity {
  ts: number;
  /** Serialized as `type` from Rust activity_type */
  type: ActivityType;
  message: string;
  meta?: Record<string, string>;
}

export interface HealthResult {
  ok: boolean;
  latencyMs: number;
  detail: string;
}

export interface CliStatus {
  found: boolean;
  path: string;
  version?: string;
  configOk: boolean;
  authPresent: boolean;
}

export interface ImportCandidate {
  id: string;
  name: string;
  baseUrl: string;
  apiKey: string;
  defaultModel: string;
  websiteUrl?: string;
  suggestedBackend: ApiBackend;
}

export interface BackupMeta {
  reason: string;
  createdAt: number;
  extra?: Record<string, string>;
}

export interface BackupInfo {
  id: string;
  reason?: string;
  createdAt?: number;
  meta?: BackupMeta;
}

export interface ProviderDraft {
  baseUrl: string;
  apiKey: string;
  apiBackend: ApiBackend;
  model: string;
}

export interface EnableProviderResult {
  providerId: string;
  backupId?: string;
  health?: HealthResult;
  postHealth?: HealthResult;
}

export interface EnableAccountResult {
  accountId: string;
  backupId?: string;
}

/** SkillScope: kebab-case */
export type SkillScope = "grok" | "claude" | "cc-switch";

export interface SkillInfo {
  name: string;
  description: string;
  path: string;
  skillMdPath: string;
  scope: SkillScope;
  isSymlink: boolean;
  linkTarget?: string;
  hasSkillMd: boolean;
  editable: boolean;
}

export interface SkillDetail {
  info: SkillInfo;
  content: string;
}

export interface SkillDraft {
  name: string;
  description: string;
  content: string;
}

/** McpTransport: kebab-case */
export type McpTransport = "stdio" | "http" | "unknown";

export interface McpServer {
  name: string;
  command?: string;
  args: string[];
  url?: string;
  env: Record<string, string>;
  headers: Record<string, string>;
  enabled: boolean;
  startupTimeoutSec?: number;
  toolTimeoutSec?: number;
  transport: McpTransport;
}

export interface McpDraft {
  name: string;
  command?: string;
  args: string[];
  url?: string;
  env: Record<string, string>;
  headers: Record<string, string>;
  enabled: boolean;
  startupTimeoutSec?: number;
  toolTimeoutSec?: number;
}

export interface McpHealthResult {
  ok: boolean;
  detail: string;
  latencyMs: number;
}

export type ApiResult<T> = {
  ok: boolean;
  data?: T;
  error?: string;
};
