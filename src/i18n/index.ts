import { useAppStore, getInitialLocale, applyAppLocale } from '@/store/useAppStore';
import { zhCN } from './locales/zh-CN';
import { en } from './locales/en';
import type { Locale, TranslationDictionary } from './types';

export * from './types';
export { getInitialLocale, applyAppLocale };

const dictionaries: Record<Locale, TranslationDictionary> = {
  'zh-CN': zhCN,
  en,
};

if (typeof window !== 'undefined') {
  applyAppLocale(getInitialLocale());
}

function getNestedValue(obj: unknown, path: string): string | undefined {
  if (!obj || typeof obj !== 'object') return undefined;
  const parts = path.split('.');
  let current: unknown = obj;
  for (const part of parts) {
    if (current && typeof current === 'object' && part in current) {
      current = (current as Record<string, unknown>)[part];
    } else {
      return undefined;
    }
  }
  return typeof current === 'string' ? current : undefined;
}

function interpolate(text: string, params?: Record<string, string | number>): string {
  if (!params) return text;
  return text.replace(/{(\w+)}/g, (_, k) => (k in params ? String(params[k]) : `{${k}}`));
}

export function translate(
  locale: Locale,
  path: string,
  params?: Record<string, string | number>,
): string {
  const dict = dictionaries[locale] || dictionaries['zh-CN'];
  const text = getNestedValue(dict, path) ?? getNestedValue(dictionaries['zh-CN'], path) ?? path;
  return interpolate(text, params);
}

export function useTranslation() {
  const locale = useAppStore((state) => state.locale);
  const setLocale = useAppStore((state) => state.setLocale);

  const t = (path: string, params?: Record<string, string | number>): string => {
    return translate(locale, path, params);
  };

  return { t, locale, setLocale };
}
