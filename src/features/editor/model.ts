import { readAutomation, defaultAutomation } from './computer-rules.ts';
import type { AutomationProfile } from '../../shared/contracts/generated';
import type { BindingRecord, Edit, KeyboardSnapshot, Rgb } from '../../shared/contracts/generated';
import { physicalKeyCodes } from '../../shared/keyboard-view/keycodes.ts';

export const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;
export const hexColor = (c: Rgb): string =>
  '#' + [c.r, c.g, c.b].map((v) => v.toString(16).padStart(2, '0')).join('');
export function parseColor(hex: string): Rgb {
  if (!/^#[0-9a-f]{6}$/i.test(hex)) throw new Error('Введите HEX в формате #AABBCC');
  return {
    r: parseInt(hex.slice(1, 3), 16),
    g: parseInt(hex.slice(3, 5), 16),
    b: parseInt(hex.slice(5, 7), 16),
  };
}
export function mixColor(a: Rgb, b: Rgb, position: number): Rgb {
  const t = Math.max(0, Math.min(1, position));
  return {
    r: Math.round(a.r + (b.r - a.r) * t),
    g: Math.round(a.g + (b.g - a.g) * t),
    b: Math.round(a.b + (b.b - a.b) * t),
  };
}
export function projectEdits(source: KeyboardSnapshot, edits: Edit[]): KeyboardSnapshot {
  const next = clone(source);
  for (const edit of edits) {
    switch (edit.kind) {
      case 'binding': {
        const layer = edit.functionLayer ? 'function' : 'base';
        for (const slot of edit.slots) {
          const old = next.keys[slot]?.[layer];
          if (old && [11, 12].includes(old.page)) {
            for (const key of next.keys)
              if (JSON.stringify(key[layer]) === JSON.stringify(old))
                key[layer] = { page: 0, parameters: [0, 0, 0] };
          }
        }
        for (const slot of edit.slots) {
          const key = next.keys[slot];
          if (key) key[layer] = clone(edit.binding);
        }
        break;
      }
      case 'actuation':
        for (const slot of edit.slots) {
          const key = next.keys[slot];
          if (key) key.actuation = { ...clone(edit.value), axisType: key.actuation.axisType };
        }
        break;
      case 'color':
        for (const slot of edit.slots) {
          const key = next.keys[slot];
          if (key) key.color = clone(edit.color);
        }
        break;
      case 'lighting':
        next.lighting = clone(edit.value);
        break;
      case 'performance':
        next.performance = clone(edit.value);
        break;
      case 'macros':
        next.macros = clone(edit.values);
        break;
      case 'dks':
        next.dks[edit.value.index] = clone(edit.value);
        break;
    }
  }
  return next;
}

export const keyChoices: Array<{ code: number; label: string; group: string }> = [
  ...Array.from({ length: 26 }, (_, i) => ({
    code: i + 4,
    label: String.fromCharCode(65 + i),
    group: 'Буквы',
  })),
  ...Array.from({ length: 9 }, (_, i) => ({ code: i + 30, label: String(i + 1), group: 'Цифры' })),
  { code: 39, label: '0', group: 'Цифры' },
  ...[
    [40, 'Enter'],
    [41, 'Esc'],
    [42, 'Backspace'],
    [43, 'Tab'],
    [44, 'Space'],
    [45, '−'],
    [46, '='],
    [47, '['],
    [48, ']'],
    [49, '\\'],
    [51, ';'],
    [52, "'"],
    [53, '`'],
    [54, ','],
    [55, '.'],
    [56, '/'],
    [57, 'Caps Lock'],
    [70, 'Print Screen'],
    [71, 'Scroll Lock'],
    [72, 'Pause'],
    [73, 'Insert'],
    [74, 'Home'],
    [75, 'Page Up'],
    [76, 'Delete'],
    [77, 'End'],
    [78, 'Page Down'],
    [79, '→'],
    [80, '←'],
    [81, '↓'],
    [82, '↑'],
    [175, 'Fn'],
    [224, 'L-Ctrl'],
    [225, 'L-Shift'],
    [226, 'L-Alt'],
    [227, 'L-Win'],
    [228, 'R-Ctrl'],
    [229, 'R-Shift'],
    [230, 'R-Alt'],
    [231, 'R-Win'],
  ].map(([code, label]) => ({ code: Number(code), label: String(label), group: 'Служебные' })),
  ...Array.from({ length: 12 }, (_, i) => ({
    code: i + 58,
    label: `F${i + 1}`,
    group: 'Функциональные',
  })),
  ...Array.from({ length: 12 }, (_, i) => ({
    code: i + 104,
    label: `F${i + 13}`,
    group: 'Функциональные',
  })),
  ...[
    [83, 'Num Lock'],
    [84, 'Num /'],
    [85, 'Num *'],
    [86, 'Num −'],
    [87, 'Num +'],
    [88, 'Num Enter'],
    [89, 'Num 1'],
    [90, 'Num 2'],
    [91, 'Num 3'],
    [92, 'Num 4'],
    [93, 'Num 5'],
    [94, 'Num 6'],
    [95, 'Num 7'],
    [96, 'Num 8'],
    [97, 'Num 9'],
    [98, 'Num 0'],
    [99, 'Num .'],
  ].map(([code, label]) => ({ code: Number(code), label: String(label), group: 'NumPad' })),
];
// White 1.17 maps these internal one-byte aliases to Consumer Page usages.
// DKS/MT/TGL use this vocabulary, while ordinary media bindings use page 3.
export const firmwareActionChoices = [
  ...keyChoices,
  ...[
    [0xb0, '⏭'],
    [0xb1, '⏮'],
    [0xb3, '⏯'],
    [0xb5, 'Mute'],
    [0xb9, 'Vol +'],
    [0xba, 'Vol −'],
  ].map(([code, label]) => ({ code: Number(code), label: String(label), group: 'Медиа' })),
];
export const keyName = (code: number) =>
  firmwareActionChoices.find((k) => k.code === code)?.label ?? `Код ${code}`;
export const deviceActions = [
  { code: 11, label: 'Следующий эффект клавиш' },
  { code: 12, label: 'Следующий цвет клавиш' },
  { code: 13, label: 'Яркость клавиш +' },
  { code: 14, label: 'Яркость клавиш −' },
  { code: 15, label: 'Скорость эффекта +' },
  { code: 16, label: 'Скорость эффекта −' },
  { code: 17, label: 'Выключить подсветку' },
  { code: 22, label: 'Блокировка Win' },
  { code: 27, label: 'Панель · следующий эффект' },
  { code: 87, label: 'Панель · глубина нажатия RT' },
  { code: 91, label: 'Панель · наложение режимов' },
];
export function bindingName(binding: BindingRecord | undefined, original: string): string {
  if (!binding || binding.page === 0) return original;
  const [a, b, c] = binding.parameters;
  switch (binding.page) {
    case 1:
      return a === 3
        ? b === 1
          ? 'Прокрутка ↑'
          : 'Прокрутка ↓'
        : ({ 1: 'ЛКМ', 2: 'ПКМ', 4: 'Средняя', 8: 'Назад', 16: 'Вперёд' }[b] ?? 'Мышь');
    case 2:
      return b === 0 && a === 0 ? original : keyName(b);
    case 3:
      return (
        (
          {
            0xe9: 'Vol +',
            0xea: 'Vol −',
            0xe2: 'Mute',
            0xcd: '⏯',
            0xb5: '⏭',
            0xb6: '⏮',
            0x192: 'Calc',
          } as Record<number, string>
        )[a + b * 256] ?? `Медиа ${a + b * 256}`
      );
    case 6:
      return `Макрос ${a + 1}`;
    case 7:
      return [a, b, c].filter(Boolean).map(keyName).join(' + ');
    case 8:
      return `DKS ${a + 1}`;
    case 9:
      return `${keyName(a)} / ${keyName(b)} · ${c * 10} мс`;
    case 10:
      return `${keyName(a)} ⇄`;
    case 11:
      return 'SOCD';
    case 12:
      return 'RS';
    case 13:
      return (
        deviceActions.find((v) => v.code === a * 65536 + b * 256 + c)?.label ??
        `Действие ${a * 65536 + b * 256 + c}`
      );
    default:
      return `Спец. ${binding.page}`;
  }
}

export const effects = [
  'Выключено',
  'Статичный',
  'При нажатии',
  'При отпускании',
  'Мерцание',
  'Падающие',
  'Цветной',
  'Дыхание',
  'Спектр',
  'От центра',
  'Прокрутка',
  'Волна',
  'Вращение',
  'Вспышка',
  'Импульс',
  'Круги',
  'Поток',
  'Пульсация',
  'Диагональ',
  'Встречные волны',
  'Свои цвета',
];

export interface LocalProfile {
  schemaVersion: 1 | 2;
  automation?: AutomationProfile;
  name: string;
  savedAt: string;
  snapshot: KeyboardSnapshot;
}
export function readLocalProfile(text: string): LocalProfile {
  if (text.length > 2_000_000) throw new Error('Файл слишком большой');
  const value: unknown = JSON.parse(text);
  if (
    !value ||
    typeof value !== 'object' ||
    !('schemaVersion' in value) ||
    ![1, 2].includes(Number(value.schemaVersion)) ||
    !('snapshot' in value)
  )
    throw new Error('Нужен профиль IO Workspace версии 1 или 2');
  const p = value as LocalProfile;
  if (
    typeof p.name !== 'string' ||
    p.name.length > 100 ||
    p.snapshot?.identity?.vendorId !== 0xc45 ||
    p.snapshot.identity.productId !== 0x80d6 ||
    p.snapshot.keys?.length !== 128
  )
    throw new Error('Профиль относится к другой клавиатуре или повреждён');
  const integer = (v: unknown, max = 255): boolean =>
    typeof v === 'number' && Number.isInteger(v) && v >= 0 && v <= max;
  const tuple = (v: unknown, length: number, max = 255): boolean =>
    Array.isArray(v) && v.length === length && v.every((n) => integer(n, max));
  const color = (v: Rgb | undefined): boolean => !!v && [v.r, v.g, v.b].every((n) => integer(n));
  const binding = (v: BindingRecord | undefined): boolean =>
    !!v && integer(v.page) && tuple(v.parameters, 3);
  if (
    typeof p.savedAt !== 'string' ||
    p.savedAt.length > 64 ||
    typeof p.snapshot.revision !== 'string' ||
    p.snapshot.revision.length > 128 ||
    typeof p.snapshot.identity.name !== 'string' ||
    p.snapshot.identity.name.length > 100 ||
    typeof p.snapshot.identity.firmware !== 'string' ||
    p.snapshot.identity.firmware.length > 32 ||
    p.snapshot.identity.frameVersion !== 0 ||
    p.snapshot.identity.rtPrecision !== 0 ||
    !integer(p.snapshot.macroBytesUsed, 60000) ||
    p.snapshot.macroWriteLimit !== 512 ||
    !Array.isArray(p.snapshot.keys)
  )
    throw new Error('Повреждены метаданные профиля');
  for (let i = 0; i < 128; i++) {
    const k = p.snapshot.keys[i];
    if (
      !k ||
      k.slot !== i ||
      !binding(k.base) ||
      !binding(k.function) ||
      !k.actuation ||
      ![k.actuation.triggerUm, k.actuation.pressUm, k.actuation.releaseUm].every((n) =>
        integer(n, 65535),
      ) ||
      ![k.actuation.rapidTrigger, k.actuation.wholeTravel, k.actuation.rampage].every(
        (n) => typeof n === 'boolean',
      ) ||
      !integer(k.actuation.axisType) ||
      !integer(k.ledId) ||
      !color(k.color)
    )
      throw new Error('Повреждена таблица клавиш');
  }
  if (
    !p.snapshot.lighting ||
    !p.snapshot.performance ||
    !Array.isArray(p.snapshot.macros) ||
    p.snapshot.macros.length > 100 ||
    !Array.isArray(p.snapshot.dks) ||
    p.snapshot.dks.length !== 64
  )
    throw new Error('В профиле не хватает блоков');
  const l = p.snapshot.lighting,
    g = p.snapshot.performance;
  if (
    !color(l.color) ||
    !color(l.secondaryColor) ||
    ![l.mode, l.colorMode, l.brightness, l.speed, l.direction, g.reportRate, g.keyDelay].every(
      (n) => integer(n),
    ) ||
    ![g.topDeadZoneUm, g.bottomDeadZoneUm].every((n) => integer(n, 65535))
  )
    throw new Error('Повреждены параметры подсветки или производительности');
  for (const [i, d] of p.snapshot.dks.entries())
    if (
      !d ||
      d.index !== i ||
      !tuple(d.thresholds, 4) ||
      !tuple(d.actions, 4) ||
      !tuple(d.states, 4)
    )
      throw new Error('Повреждена таблица DKS');
  const ids = new Set<number>();
  let stepCount = 0;
  for (const m of p.snapshot.macros) {
    if (
      !m ||
      !integer(m.id, 99) ||
      ids.has(m.id) ||
      !Array.isArray(m.steps) ||
      m.steps.length > 15000
    )
      throw new Error('Повреждён каталог макросов');
    ids.add(m.id);
    stepCount += m.steps.length;
    for (const s of m.steps)
      if (
        !s ||
        !integer(s.keyCode) ||
        !integer(s.kind, 7) ||
        !integer(s.delayMs, 65535) ||
        typeof s.pressed !== 'boolean'
      )
        throw new Error('Повреждено действие макроса');
  }
  if (stepCount > 15000) throw new Error('Превышен размер каталога макросов');
  p.automation = p.schemaVersion === 2 ? readAutomation(p.automation) : defaultAutomation();
  return p;
}

export function profileEdits(current: KeyboardSnapshot, target: KeyboardSnapshot): Edit[] {
  const edits: Edit[] = [];
  const changed = (a: unknown, b: unknown) => JSON.stringify(a) !== JSON.stringify(b);
  if (changed(current.macros, target.macros)) edits.push({ kind: 'macros', values: target.macros });
  for (const d of target.dks)
    if (changed(current.dks[d.index], d)) edits.push({ kind: 'dks', value: d });
  const pairs: Edit[] = [];
  const seen = new Set<string>();
  for (const key of target.keys.filter((k) => physicalKeyCodes[k.slot] !== undefined)) {
    const old = current.keys[key.slot];
    if (!old) continue;
    for (const layer of ['base', 'function'] as const) {
      const binding = key[layer];
      if (changed(old[layer], binding)) {
        if ([11, 12].includes(binding.page)) {
          const pair = target.keys.filter(
            (k) => JSON.stringify(k[layer]) === JSON.stringify(binding),
          );
          if (pair.length !== 2) throw new Error('Неполная пара RS/SOCD в профиле');
          const first = pair.find((k) => physicalKeyCodes[k.slot] === binding.parameters[1]);
          const second = pair.find((k) => physicalKeyCodes[k.slot] === binding.parameters[2]);
          if (!first || !second || first.slot === second.slot)
            throw new Error('Адреса пары RS/SOCD не совпадают с раскладкой');
          const id = layer + ':' + first.slot + ':' + second.slot;
          if (!seen.has(id)) {
            seen.add(id);
            pairs.push({
              kind: 'binding',
              slots: [first.slot, second.slot],
              functionLayer: layer === 'function',
              binding,
            });
          }
        } else
          edits.push({
            kind: 'binding',
            slots: [key.slot],
            functionLayer: layer === 'function',
            binding,
          });
      }
    }
    if (changed(old.actuation, key.actuation))
      edits.push({ kind: 'actuation', slots: [key.slot], value: key.actuation });
    if (changed(old.color, key.color))
      edits.push({ kind: 'color', slots: [key.slot], color: key.color });
  }
  edits.push(...pairs);
  if (changed(current.lighting, target.lighting))
    edits.push({ kind: 'lighting', value: target.lighting });
  if (changed(current.performance, target.performance))
    edits.push({ kind: 'performance', value: target.performance });
  return edits;
}
