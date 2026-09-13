import { ref, watchEffect } from 'vue';
import en from './en.json' with { type: 'json' };
type Locale = 'ru' | 'en';
type Theme = 'system' | 'light' | 'dark';
function stored(name: string) {
  try {
    return localStorage.getItem(name);
  } catch {
    return null;
  }
}
export const locale = ref<Locale>(stored('io.locale') === 'en' ? 'en' : 'ru');
export const theme = ref<Theme>(
  (['light', 'dark'].includes(stored('io.theme') ?? '') ? stored('io.theme') : 'system') as Theme,
);
export function t(text: string): string {
  if (locale.value === 'ru') return text;
  const catalog = en as Record<string, string>;
  if (catalog[text]) return catalog[text];
  const numbered = /^(Макрос|Слот|Медиа|Код|Действие|Спец\.) (\d+)(.*)$/.exec(text);
  if (numbered)
    return `${({ Макрос: 'Macro', Слот: 'Entry', Медиа: 'Media', Код: 'Code', Действие: 'Action', 'Спец.': 'Special' } as Record<string, string>)[numbered[1]!]} ${numbered[2]}${numbered[3]}`;
  if (/^\d+ клавиш$/.test(text)) return text.replace('клавиш', 'keys');
  return text;
}
export const mm = (um: number) =>
  (um / 1000).toLocaleString(locale.value, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
export function initPreferences() {
  watchEffect(() => {
    document.documentElement.lang = locale.value;
    document.documentElement.dataset.theme = theme.value;
    try {
      localStorage.setItem('io.locale', locale.value);
      localStorage.setItem('io.theme', theme.value);
    } catch {
      /* Preferences still work for this session. */
    }
  });
}
