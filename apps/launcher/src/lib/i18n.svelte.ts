// Interface language. English is the source text: every string in the UI is written in English and passed through `tr`,
// which returns the translation for the current language (Italian in ./i18n/it.ts, Simplified Chinese in ./i18n/zh.ts).
// `scripts/check-i18n.mjs` (part of `pnpm check`) fails when an English string has no Italian or Chinese translation.
// Pattern from the maintainer's Deluxe8 launcher.

import { it } from './i18n/it';
import { zh } from './i18n/zh';
import type { UiLanguage } from './types';

export type Locale = 'en' | 'it' | 'zh';

const dictionaries: Record<Locale, Record<string, string> | null> = { en: null, it, zh };

let current = $state<Locale>('en');

/**
 * The text `en` in the interface language, with `{name}` placeholders filled from `params`. Reactive in templates.
 * A context prefix tells apart identical English words that translate differently: `tr('vsync|Off')` shows "Off" in
 * English and the `vsync|Off` translation in Italian.
 */
export function tr(en: string, params?: Record<string, string | number>): string {
  const bar = en.indexOf('|');
  let text = dictionaries[current]?.[en] ?? (bar >= 0 ? en.slice(bar + 1) : en);
  if (params) {
    for (const [key, value] of Object.entries(params)) text = text.replaceAll(`{${key}}`, String(value));
  }
  return text;
}

/** Catalogue labels and help come from catalog.json (English); they are looked up in the same dictionary. */
export function trc(en: string): string {
  return dictionaries[current]?.[en] ?? en;
}

export function setLocale(l: Locale) {
  current = l;
  if (typeof document !== 'undefined') document.documentElement.lang = l === 'zh' ? 'zh-Hans' : l;
}

/**
 * "auto" means Italian for players in Italy (or San Marino, Vatican City) or with an Italian desktop, Simplified Chinese
 * with a Chinese desktop outside Taiwan, Hong Kong and Macau (as the game's own menu decides), English otherwise.
 */
export function resolveLocale(setting: UiLanguage, system: { language: string; country: string } | null | undefined): Locale {
  if (setting === 'en' || setting === 'it' || setting === 'zh') return setting;
  if (!system) return 'en';
  if (system.language === 'zh') return ['TW', 'HK', 'MO'].includes(system.country) ? 'en' : 'zh';
  return system.language === 'it' || ['IT', 'SM', 'VA'].includes(system.country) ? 'it' : 'en';
}
