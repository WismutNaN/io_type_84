import type {
  ActionDefinition,
  ActionCommand,
  HoldRepeat,
  AutomationProfile,
  DepthRule,
} from '../../shared/contracts/generated';
import { keyboardKeys } from '../../shared/keyboard-view/layout.ts';
export const computerActions = [
  { id: 'volumeUp', label: 'Громче', short: 'Vol +' },
  { id: 'volumeDown', label: 'Тише', short: 'Vol −' },
  { id: 'mute', label: 'Без звука', short: 'Mute' },
  { id: 'playPause', label: 'Воспроизведение / пауза', short: '⏯' },
] as const;
export const emptyPlatforms = () => ({ windows: null, linux: null, macos: null });
export const defaultAutomation = (): AutomationProfile => ({
  actions: [
    ...computerActions.map((a) => ({
      id: a.id,
      name: a.label,
      platformCommands: emptyPlatforms(),
      command: { kind: 'media' as const, action: a.id },
    })),
    {
      id: 'word',
      name: 'Открыть Word',
      platformCommands: emptyPlatforms(),
      command: { kind: 'application', application: 'word' },
    },
    {
      id: 'switch-window',
      name: 'Следующее окно',
      platformCommands: emptyPlatforms(),
      command: { kind: 'key', key: 43, modifiers: 4 },
    },
  ],
  gestures: [],
  depthChoices: [],
});
export function actionLabel(action: ActionDefinition | undefined): string {
  if (!action) return '—';
  if (action.command.kind === 'media')
    return (
      computerActions.find((a) => a.id === (action.command as { action: string }).action)?.short ??
      action.name
    );
  return action.name;
}
export const repeatableCommand = (c: ActionCommand) =>
  c.kind === 'key' || (c.kind === 'media' && ['volumeUp', 'volumeDown'].includes(c.action));
export const repeatableAction = (a: ActionDefinition | undefined) =>
  !!a &&
  repeatableCommand(a.command) &&
  Object.values(a.platformCommands).every((c) => c === null || repeatableCommand(c));
export const supportedKey = (key: number) =>
  Number.isInteger(key) &&
  ((key >= 4 && key <= 69) || (key >= 73 && key <= 82) || (key >= 224 && key <= 231));
export function readAutomation(value: unknown): AutomationProfile {
  const fail = () => {
    throw new Error('Проверьте каталог действий и условия жестов.');
  };
  if (!value || typeof value !== 'object') return fail();
  const p = JSON.parse(JSON.stringify(value)) as AutomationProfile;
  if (p.depthChoices === undefined) p.depthChoices = [];
  if (
    !Array.isArray(p.actions) ||
    !Array.isArray(p.gestures) ||
    p.actions.length > 256 ||
    !Array.isArray(p.depthChoices) ||
    p.gestures.length + p.depthChoices.length > 256
  )
    return fail();
  const integer = (v: unknown, max: number) =>
    typeof v === 'number' && Number.isInteger(v) && v >= 0 && v <= max;
  const id = (v: unknown) => typeof v === 'string' && /^[a-zA-Z0-9_.-]{1,80}$/.test(v);
  const text = (v: unknown) => typeof v === 'string' && [...v].length <= 2000 && !v.includes('\0');
  const command = (c: any, step = false): boolean => {
    if (!c || typeof c !== 'object') return false;
    switch (c.kind) {
      case 'media':
        return computerActions.some((a) => a.id === c.action);
      case 'key':
        return supportedKey(c.key) && integer(c.modifiers, 255);
      case 'text':
        return text(c.text);
      case 'application':
        return !step && ['word', 'notepad', 'calculator'].includes(c.application);
      case 'delay':
        return step && integer(c.ms, 10000);
      case 'macro':
        return (
          !step &&
          Array.isArray(c.steps) &&
          c.steps.length > 0 &&
          c.steps.length <= 128 &&
          c.steps.every((s: unknown) => command(s, true)) &&
          c.steps.reduce((n: number, s: any) => n + (s.kind === 'delay' ? s.ms : 0), 0) <= 30000 &&
          c.steps.reduce(
            (n: number, s: any) => n + (s.kind === 'text' ? [...s.text].length : 0),
            0,
          ) <= 4000
        );
      default:
        return false;
    }
  };
  const ids = new Set<string>();
  for (const a of p.actions) {
    if (
      !a ||
      !id(a.id) ||
      ids.has(a.id) ||
      typeof a.name !== 'string' ||
      !a.name.trim() ||
      [...a.name].length > 100 ||
      !command(a.command)
    )
      return fail();
    if (!a.platformCommands) a.platformCommands = emptyPlatforms();
    for (const os of ['windows', 'linux', 'macos'] as const) {
      const variant = a.platformCommands[os];
      if (variant !== null && !command(variant)) return fail();
    }
    ids.add(a.id);
  }
  const gestures = new Set<string>();
  for (const r of p.gestures) {
    if (
      !r ||
      !id(r.id) ||
      gestures.has(r.id) ||
      !ids.has(r.actionId) ||
      !Array.isArray(r.slots) ||
      !r.slots.length ||
      r.slots.length > 8 ||
      new Set(r.slots).size !== r.slots.length ||
      !r.slots.every((s) => keyboardKeys.some((k) => k.slot === s)) ||
      !integer(r.thresholdUm, 3200) ||
      r.thresholdUm < 300 ||
      !integer(r.releaseUm, 3100) ||
      r.releaseUm + 100 > r.thresholdUm ||
      !integer(r.holdMs, 10000)
    )
      return fail();
    gestures.add(r.id);
  }
  const owned = new Set<number>();
  for (const r of p.depthChoices) {
    if (!r || typeof r !== 'object') return fail();
    if (r.deepRepeat === undefined) r.deepRepeat = null;
    if (
      r.deepRepeat !== null &&
      (!integer(r.deepRepeat.delayMs, 2000) ||
        r.deepRepeat.delayMs < 100 ||
        !integer(r.deepRepeat.intervalMs, 1000) ||
        r.deepRepeat.intervalMs < 50 ||
        !repeatableAction(p.actions.find((a) => a.id === r.deepActionId)))
    )
      return fail();
    if (
      !r ||
      !id(r.id) ||
      gestures.has(r.id) ||
      !ids.has(r.lightActionId) ||
      !ids.has(r.deepActionId) ||
      !keyboardKeys.some((k) => k.slot === r.slot) ||
      owned.has(r.slot) ||
      !integer(r.lightUm, 3000) ||
      r.lightUm < 300 ||
      !integer(r.deepUm, 3200) ||
      r.deepUm < r.lightUm + 100 ||
      !integer(r.releaseUm, 2900) ||
      r.releaseUm + 100 > r.lightUm ||
      p.gestures.some((g) => g.slots.includes(r.slot))
    )
      return fail();
    owned.add(r.slot);
    gestures.add(r.id);
  }
  return JSON.parse(JSON.stringify(p));
}
export function migrateDepthRules(value: unknown): AutomationProfile {
  if (!Array.isArray(value) || value.length > 84) throw new Error('Неверные правила');
  const p = defaultAutomation();
  p.gestures = (value as DepthRule[]).map((r, i) => ({
    id: `legacy-${i}`,
    slots: [r.slot],
    thresholdUm: r.thresholdUm,
    releaseUm: r.releaseUm,
    holdMs: 0,
    actionId: r.action,
  }));
  return readAutomation(p);
}

export const hasRules = (p: AutomationProfile) => p.gestures.length + p.depthChoices.length > 0;
export function pageDepthChoice(
  profile: AutomationProfile,
  slot: number,
  deepActionId: string,
  deepUm = 3000,
  lightActionId?: string,
  lightUm = 600,
  deepRepeat?: HoldRepeat | null,
): AutomationProfile {
  const next = readAutomation(profile);
  const key = slot === 105 ? 75 : 78;
  const id = `page-${slot}`;
  if (!lightActionId && !next.actions.some((a) => a.id === id))
    next.actions.push({
      id,
      name: slot === 105 ? 'Page Up' : 'Page Down',
      platformCommands: emptyPlatforms(),
      command: { kind: 'key', key, modifiers: 0 },
    });
  const existing = next.depthChoices.find((r) => r.slot === slot);
  next.gestures = next.gestures.filter((g) => !g.slots.includes(slot));
  next.depthChoices = next.depthChoices.filter((r) => r.slot !== slot);
  next.depthChoices.push({
    id: existing?.id ?? `depth-${slot}`,
    slot,
    lightUm,
    deepUm,
    releaseUm: 200,
    lightActionId: lightActionId ?? id,
    deepActionId,
    deepRepeat:
      deepRepeat === undefined
        ? (existing?.deepRepeat ??
          (existing
            ? null
            : repeatableAction(next.actions.find((a) => a.id === deepActionId))
              ? { delayMs: 350, intervalMs: 80 }
              : null))
        : deepRepeat,
  });
  return readAutomation(next);
}
