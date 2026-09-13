import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { DepthRule } from '../../shared/contracts/generated';
import { keyboardKeys } from '../../shared/keyboard-view/layout';
import { clone } from './model';
export const computerActions = [
  { id: 'volumeUp', label: 'Громче', short: 'Vol +' },
  { id: 'volumeDown', label: 'Тише', short: 'Vol −' },
  { id: 'mute', label: 'Без звука', short: 'Mute' },
  { id: 'playPause', label: 'Воспроизведение / пауза', short: '⏯' },
] as const;
export function readRules(value: unknown): DepthRule[] {
  if (!Array.isArray(value) || value.length > 84) throw new Error('Неверные правила');
  const slots = new Set<number>();
  for (const r of value) {
    if (
      !r ||
      !keyboardKeys.some((k) => k.slot === r.slot) ||
      slots.has(r.slot) ||
      !Number.isInteger(r.thresholdUm) ||
      !Number.isInteger(r.releaseUm) ||
      r.thresholdUm < 300 ||
      r.thresholdUm > 3200 ||
      r.releaseUm < 0 ||
      r.releaseUm + 100 > r.thresholdUm ||
      !computerActions.some((a) => a.id === r.action)
    )
      throw new Error('Неверные правила');
    slots.add(r.slot);
  }
  return clone(value);
}
export function useComputerRules() {
  const rules = ref<DepthRule[]>([]),
    ruleMessage = ref('');
  try {
    rules.value = readRules(JSON.parse(localStorage.getItem('io.rules.v1') ?? '[]'));
  } catch {
    ruleMessage.value = 'Не удалось прочитать программные действия.';
  }
  async function save(next: DepthRule[], native: boolean) {
    try {
      const validated = readRules(next);
      // Изменение/удаление условия сначала прекращает исполнение старой версии.
      if (native) await invoke('configure_rules', { rules: [], enabled: false });
      localStorage.setItem('io.rules.v1', JSON.stringify(validated));
      rules.value = validated;
      ruleMessage.value = 'Действия сохранены. Для запуска включите их явно.';
    } catch (e) {
      ruleMessage.value =
        typeof e === 'object' && e && 'message' in e ? String(e.message) : String(e);
    }
  }
  async function enable(enabled: boolean) {
    try {
      await invoke('configure_rules', { rules: clone(rules.value), enabled });
      ruleMessage.value = '';
    } catch (e) {
      ruleMessage.value =
        typeof e === 'object' && e && 'message' in e ? String(e.message) : String(e);
    }
  }
  return { rules, ruleMessage, save, enable };
}
