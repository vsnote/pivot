export type Protocol = 'anthropic' | 'openai' | 'gemini' | 'openrouter' | 'ollama' | 'chatgpt';

export interface Provider {
  id: string;
  name: string;
  protocol: Protocol;
  baseUrl: string;
  presetId?: string | null;
  authType?: string | null;
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

export type ClientType = 'claude_code' | 'codex_cli' | 'gemini_cli' | 'pi' | 'openai_api' | 'anthropic_api' | 'generic';

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

export type RouteLayer = 'prefix' | 'role' | 'rule' | 'exact' | 'normalized' | 'default';

/** One layer the gateway tried while resolving a model name. */
export interface RouteStep {
  layer: RouteLayer;
  matched: boolean;
  ruleId?: string;
  detail: string;
}

export interface RouteExplanation {
  incoming: string;
  steps: RouteStep[];
  plan: RoutePlan | null;
  error: string | null;
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
  id: 'claude_code' | 'codex_cli' | 'gemini_cli' | 'pi';
  name: string;
  installed: boolean;
  configPath: string;
  isIntercepted: boolean;
  backupPath?: string;
  originalEndpoint?: string;
  conflict?: string;
  thirdParty?: ThirdPartyConfig;
  isOverwritten?: boolean;
  model?: string | null;
}

export interface SubscriptionAccount {
  id: string;
  providerId: string;
  accountType: string;
  email?: string | null;
  accessToken: string;
  refreshToken?: string | null;
  expiresAt: number;
  cooldownUntil: number;
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface QuotaBucket {
  id: string;
  label: string;
  remainingFraction: number; // 0.0 ~ 1.0
  resetTime?: string | null;
  isExhausted: boolean;
}

export interface ResetCredit {
  id: string;
  status: string;
  expiresAt: string;
  grantedAt?: string | null;
}

export interface AccountQuota {
  accountId: string;
  updatedAt: number;
  buckets: QuotaBucket[];
  resetCredits?: ResetCredit[];
  error?: string | null;
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
  authType?: string | null;
}

export type SettingsSection = 'clients' | 'gateway' | 'logs' | 'about';

export type ActiveTab = 'dashboard' | 'providers' | 'clients' | 'interceptor' | 'logs' | 'settings';
