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
  | "skill";

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
}

export interface Account {
  id: string;
  name: string;
  email?: string;
  labelColor: string;
  status: AccountStatus;
  lastUsedAt?: number;
  createdAt: number;
}

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

export type ApiResult<T> = {
  ok: boolean;
  data?: T;
  error?: string;
};
