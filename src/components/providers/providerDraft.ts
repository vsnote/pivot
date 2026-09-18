import type { Preset, Provider } from '@/types';

export const uniqueValues = (values: string[]) => [...new Set(values.map((v) => v.trim()).filter(Boolean))];

export function editablePresetUrl(preset: Preset) {
  return preset.category === '开源 / 本地部署' || preset.baseUrl.includes('YOUR_ACCOUNT_ID');
}

export function createProviderDraft(provider: Provider | null, presets: Preset[]): Provider {
  if (provider) {
    // Only migrate an unambiguous legacy preset; customized endpoints remain custom.
    const preset = provider.presetId === undefined ? presets.find((p) => p.id === provider.id &&
      p.protocol === provider.protocol && p.baseUrl.replace(/\/+$/, '') === provider.baseUrl.replace(/\/+$/, '')) : undefined;
    return { ...provider, presetId: provider.presetId ?? preset?.id ?? null,
      apiKeys: [...provider.apiKeys], models: uniqueValues(provider.models),
      defaultModel: provider.defaultModel ?? null, isDefault: provider.isDefault ?? false };
  }
  return { id: `prov_${crypto.randomUUID()}`, name: '', protocol: 'openai', baseUrl: '',
    presetId: null, apiKeys: [], models: [], defaultModel: null, isDefault: false,
    enabled: true, priority: 10, weight: 1, timeoutSecs: 60 };
}

export function applyProviderPreset(draft: Provider, preset?: Preset): Provider {
  // Changing the service clears everything that belonged to the previous one,
  // including the default model and the catch-all role.
  return { ...draft, presetId: preset?.id ?? null, name: preset?.name ?? '',
    protocol: preset?.protocol ?? 'openai', baseUrl: preset?.baseUrl ?? '',
    apiKeys: [], models: [], defaultModel: null, isDefault: false, latencyMs: undefined };
}

export function validateBaseUrl(value: string): string | null {
  try {
    const url = new URL(value.trim());
    if (!['http:', 'https:'].includes(url.protocol) || !url.hostname) return '请输入 http:// 或 https:// 开头的接口地址';
    if (url.username || url.password || url.search || url.hash) return '地址不能包含账号、密码、查询参数或锚点，请将密钥填在下方';
    if (value.includes('YOUR_ACCOUNT_ID')) return '请将 YOUR_ACCOUNT_ID 替换为你的账号 ID';
    if (/\/(chat\/completions|messages|models)\/?$/.test(url.pathname)) return '请填写基础接口地址，不要包含 /chat/completions、/messages 或 /models';
    return null;
  } catch {
    return '请输入有效的接口地址，例如 https://api.example.com/v1';
  }
}

export function prepareProvider(draft: Provider): Provider {
  return { ...draft, name: draft.name.trim(), baseUrl: draft.baseUrl.trim().replace(/\/+$/, ''),
    apiKeys: uniqueValues(draft.apiKeys), models: uniqueValues(draft.models),
    defaultModel: draft.defaultModel?.trim() || null, isDefault: !!draft.isDefault,
    updatedAt: new Date().toISOString() };
}
