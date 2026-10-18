import { createI18n } from 'vue-i18n';
import { watch } from 'vue';
import { readPreference, writePreference } from '../shared/preferences';

interface LanguageFile {
  meta: { locale: string; nativeName: string; direction: 'ltr' | 'rtl' };
  messages: Record<string, string | Record<string, string>>;
}

const files = import.meta.glob<LanguageFile>('./locales/*.json', { eager: true, import: 'default' });
export const languages = Object.values(files).map(file => file.meta);
const messages = Object.fromEntries(Object.values(files).map(file => [file.meta.locale, file.messages]));

function initialLocale(): string {
  const saved = readPreference('locale.v1');
  if (saved && messages[saved]) return saved;
  for (const wanted of navigator.languages) {
    const match = languages.find(lang => lang.locale.toLowerCase() === wanted.toLowerCase())
      ?? languages.find(lang => lang.locale.split('-')[0] === wanted.split('-')[0]);
    if (match) return match.locale;
  }
  return 'en';
}

export const i18n = createI18n({ legacy: false, locale: initialLocale(), fallbackLocale: 'en', messages });

watch(i18n.global.locale, locale => {
  const meta = languages.find(lang => lang.locale === locale);
  document.documentElement.lang = locale;
  document.documentElement.dir = meta?.direction ?? 'ltr';
}, { immediate: true });

export function setLocale(locale: string) {
  if (!messages[locale]) return;
  i18n.global.locale.value = locale;
  writePreference('locale.v1', locale);
}
