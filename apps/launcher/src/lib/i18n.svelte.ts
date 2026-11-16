// Interface language. English is the source text: every string in the UI is written in English and passed through `tr`,
// which returns the translation for the current language (Italian lives in ./i18n/it.ts). `scripts/check-i18n.mjs`
// (part of `pnpm check`) fails when an English string has no Italian translation.
// Pattern from the maintainer's Deluxe8 launcher.

import { it } from './i18n/it';
import type { UiLanguage } from './types';

export type Locale = 'en' | 'it';

const dictionaries: Record<Locale, Record<string, string> | null> = { en: null, it };

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

export function locale(): Locale {
  return current;
}

export function setLocale(l: Locale) {
  current = l;
  if (typeof document !== 'undefined') document.documentElement.lang = l;
}

/** "auto" means Italian for players in Italy (or San Marino, Vatican City) or with an Italian desktop, English otherwise. */
export function resolveLocale(setting: UiLanguage, system: { language: string; country: string } | null | undefined): Locale {
  if (setting === 'en' || setting === 'it') return setting;
  if (!system) return 'en';
  return system.language === 'it' || ['IT', 'SM', 'VA'].includes(system.country) ? 'it' : 'en';
}
