import type { BindingRecord, Edit, KeyboardSnapshot } from '../../shared/contracts/generated';
import { physicalKeyCodes } from '../../shared/keyboard-view/keycodes.ts';
import { clone, deviceActions, profileEdits } from './model.ts';

type ObjectData = Record<string, unknown>;
function object(v: unknown): ObjectData {
  if (!v || typeof v !== 'object' || Array.isArray(v))
    throw new Error('Ожидался объект профиля IO Vision');
  return v as ObjectData;
}
function number(v: unknown, max = 255): number {
  if (typeof v !== 'number' || !Number.isInteger(v) || v < 0 || v > max)
    throw new Error('Недопустимое число в профиле IO Vision');
  return v;
}
function um(v: unknown, max: number): number {
  if (
    typeof v !== 'number' ||
    !Number.isFinite(v) ||
    v < 0 ||
    v > max ||
    Math.abs(v * 100 - Math.round(v * 100)) > 0.00001
  )
    throw new Error('Некорректная глубина нажатия в профиле');
  return Math.round(v * 1000);
}
function array(v: unknown, size: number): unknown[] {
  if (!Array.isArray(v) || v.length !== size)
    throw new Error('Размер таблицы профиля не соответствует IO White');
  return v;
}
function binding(v: ObjectData): BindingRecord | null {
  const param = (key: string) => number(v[key] ?? 0);
  switch (v.page) {
    case 'FUNC': {
      const code = number(v.value, 0xffffff);
      return deviceActions.some((a) => a.code === code)
        ? { page: 13, parameters: [0, 0, code] }
        : null;
    }
    case 'DEFAULT':
      return { page: 0, parameters: [0, 0, 0] };
    case 'KEYBOARD':
      return { page: 2, parameters: [0, param('value'), 0] };
    case 'MOUSE':
      return { page: 1, parameters: [param('param1') || 1, param('value'), 0] };
    case 'CONSUMER_KEY': {
      const code = number(v.value, 65535);
      return { page: 3, parameters: [code & 255, code >> 8, 0] };
    }
    case 'TGL':
      return { page: 10, parameters: [param('value'), 0, 0] };
    case 'MT':
    case 'SOCD':
    case 'RS':
    case 'CB':
      return {
        page: ({ MT: 9, SOCD: 11, RS: 12, CB: 7 } as Record<string, number>)[String(v.page)]!,
        parameters: [param('param1'), param('param2'), param('param3')],
      };
    // Макросы/DKS требуют связанной таблицы; неизвестные назначения сохраняются в исходном снимке.
    default:
      return null;
  }
}

export function importVisionProfile(
  text: string,
  current: KeyboardSnapshot,
): { name: string; edits: Edit[]; warnings: string[] } {
  if (text.length > 2_000_000) throw new Error('Файл слишком большой');
  const root = object(JSON.parse(text));
  if (root.deviceId !== '3141:32982:IO Type 84 Magnetic White')
    throw new Error('Профиль IO Vision относится к другой модели');
  const p = object(root.profile),
    target = clone(current),
    warnings: string[] = [];
  let skippedBindings = 0,
    missingBindings = 0,
    zeroThresholds = 0;
  for (const [field, layer] of [
    ['keyList', 'base'],
    ['fnKeyList', 'function'],
  ] as const) {
    if (!(field in p)) continue;
    const rows = array(p[field], 6),
      seen = new Set<number>();
    for (const row of rows) {
      if (!Array.isArray(row) || row.length > 24) throw new Error('Некорректный ряд клавиш');
      for (const raw of row) {
        const key = object(raw),
          slot = number(key.value, 127);
        if (
          physicalKeyCodes[slot] === undefined ||
          seen.has(slot) ||
          key.keyCode !== physicalKeyCodes[slot]
        )
          throw new Error('Адреса профиля не совпадают с раскладкой');
        seen.add(slot);
        if (key.userKey === undefined) {
          missingBindings++;
          continue;
        }
        const b = binding(object(key.userKey));
        if (b) target.keys[slot]![layer] = b;
        else skippedBindings++;
      }
    }
    if (seen.size !== 84) throw new Error('В профиле должна быть полная раскладка из 84 клавиш');
  }
  if (p.magneticAxisRT !== undefined) {
    array(p.magneticAxisRT, 128).forEach((v, slot) => {
      if (physicalKeyCodes[slot] === undefined) return;
      const a = object(v);
      const trigger = um(a.triggerKeyStroke, 3.2),
        press = um(a.pressRT, 3.2),
        release = um(a.releaseRT, 3.2);
      if (trigger < 100) {
        zeroThresholds++;
        return;
      }
      if (typeof a.isWholeFast !== 'boolean' || typeof a.isRampageMode !== 'boolean')
        throw new Error('Некорректные флаги Rapid Trigger');
      target.keys[slot]!.actuation = {
        ...target.keys[slot]!.actuation,
        triggerUm: trigger,
        pressUm: press,
        releaseUm: release,
        rapidTrigger: press > 0 || release > 0,
        wholeTravel: a.isWholeFast,
        rampage: a.isRampageMode,
      };
    });
  }
  if (p.customLedData !== undefined) {
    const seen = new Set<number>();
    array(p.customLedData, 128).forEach((v) => {
      const c = object(v),
        id = number(c.ledId, 127);
      if (seen.has(id)) throw new Error('Повторяющийся LED-адрес');
      seen.add(id);
      const key = target.keys.find((k) => k.ledId === id);
      if (key && physicalKeyCodes[key.slot] !== undefined)
        key.color = { r: number(c.red), g: number(c.green), b: number(c.blue) };
    });
  }
  if (p.ledEffect !== undefined) {
    const l = object(p.ledEffect);
    target.lighting = {
      mode: number(l.mode, 20),
      color: { r: number(l.red), g: number(l.green), b: number(l.blue) },
      secondaryColor: {
        r: number(l.secondaryRed),
        g: number(l.secondaryGreen),
        b: number(l.secondaryBlue),
      },
      colorMode: number(l.colorMode, 1),
      brightness: number(l.brightness, 5),
      speed: number(l.speed, 5),
      direction: number(l.direction, 1),
    };
  }
  if (p.gameModeInfo !== undefined) {
    const g = object(p.gameModeInfo),
      rate = number(g.reportRate);
    if (![3, 5, 6].includes(rate)) throw new Error('Неизвестная частота опроса IO White');
    target.performance = {
      ...target.performance,
      reportRate: rate,
      topDeadZoneUm: um(g.topDeadZone, 0.5),
      bottomDeadZoneUm: um(g.bottomDeadZone, 0.5),
    };
  }
  if (missingBindings)
    warnings.push(`${missingBindings} клавиш без userKey: исходные назначения сохранены.`);
  if (skippedBindings)
    warnings.push(
      `${skippedBindings} специальных назначений не перенесено: формат ещё не поддержан импортом.`,
    );
  if (zeroThresholds)
    warnings.push(
      `${zeroThresholds} нулевых/слишком малых порогов пропущено. Исходный ход сохранён.`,
    );
  if (
    (Array.isArray(p.macroDataList) && p.macroDataList.length) ||
    (Array.isArray(p.magneticAxisDKS) && p.magneticAxisDKS.length)
  )
    warnings.push(
      'Каталоги макросов и DKS из сайта пока не импортируются. Используйте их редакторы или собственный JSON-профиль.',
    );
  warnings.push(
    'Отсутствующие и неизвестные поля сохранены из исходного снимка. Импорт добавлен только в черновик.',
  );
  return {
    name: typeof p.profileName === 'string' ? p.profileName.slice(0, 100) : 'Импорт IO Vision',
    edits: profileEdits(current, target),
    warnings,
  };
}
