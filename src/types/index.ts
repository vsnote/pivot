export type Protocol = 'anthropic' | 'openai' | 'gemini' | 'openrouter' | 'ollama';

export interface Provider {
  id: string;
  name: string;
  protocol: Protocol;
  baseUrl: string;
  presetId?: string | null;
  apiKeys: string[];
  models: string[];
  /** Model to ask for when this provider does not declare the requested one. */
  defaultModel?: string | null;
  /** Catch-all provider for model names no enabled provider declares. */
  isDefault?: boolean;
  enabled: boolean;
  priority: number;
  weight: number;
  timeoutSecs: number;
  latencyMs?: number;
  createdAt?: string;
  updatedAt?: string;
}

export interface RouteRule {
  id: string;
  pattern: string; // e.g. "claude-*", "gpt-*", "deepseek/*", "*"
  targetProviderId: string;
  targetModel?: string; // If empty, forwards original model name
  fallbackProviderIds: string[]; // List of fallback provider IDs
  enabled: boolean;
  priority: number; // Higher number = checked first
  description?: string;
}

export type ClientType = 'claude_code' | 'codex_cli' | 'gemini_cli' | 'openai_api' | 'anthropic_api' | 'generic';

export interface LogAttempt {
  providerId: string;
  providerName: string;
  model: string;
  status: number;
  error?: string | null;
  latencyMs: number;
}

export interface RequestLog {
  id: string;
  timestamp: string;
  method: string;
  path: string;
  clientType: ClientType;
  incomingModel: string;
  routedProviderId: string;
  routedProviderName?: string;
  routedModel: string;
  statusCode: number;
  latencyMs: number;
  promptTokens: number;
  completionTokens: number;
  errorMessage?: string;
  isStream: boolean;
  fallbackUsed?: boolean;
  /** Every upstream attempt in order, oldest first. */
  attempts?: LogAttempt[];
}

/** One provider/model decision taken by the gateway resolver. */
export interface PlannedCandidate {
  providerId: string;
  model: string;
}

export interface RoutePlan {
  primary: PlannedCandidate;
  fallbacks: PlannedCandidate[];
}

export interface GatewayStats {
  isRunning: boolean;
  host: string;
  port: number;
  activeRequests: number;
  totalRequests: number;
  totalPromptTokens: number;
  totalCompletionTokens: number;
  errorCount: number;
  uptimeSecs: number;
}

export interface ThirdPartyConfig {
  endpoint: string;
  authToken?: string;
  suggestedName: string;
  protocol: string;
  models: string[];
}

export interface InterceptorStatus {
  id: 'claude_code' | 'codex_cli' | 'gemini_cli';
  name: string;
  installed: boolean;
  configPath: string;
  isIntercepted: boolean;
  backupPath?: string;
  originalEndpoint?: string;
  conflict?: string;
  thirdParty?: ThirdPartyConfig;
  isOverwritten?: boolean;
}

export interface Preset {
  id: string;
  name: string;
  protocol: Protocol;
  baseUrl: string;
  defaultModels: string[];
  description: string;
  icon?: string;
  category: string;
}

export type SettingsSection = 'clients' | 'routes' | 'gateway' | 'logs' | 'about';

export type ActiveTab = 'dashboard' | 'providers' | 'clients' | 'routes' | 'interceptor' | 'logs' | 'settings';
