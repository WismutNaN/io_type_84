import type { LightingSettings, Rgb } from '../../shared/contracts/generated';

// Иллюстрации характера эффектов, не эмулятор неизвестного firmware renderer.
export const effectGroups = [
  { name: 'Постоянный свет', modes: [0, 1, 7, 8, 20] },
  { name: 'Движение', modes: [5, 6, 9, 10, 11, 12, 16, 18, 19] },
  { name: 'Вспышки и отклик', modes: [2, 3, 4, 13, 14, 15, 17] },
];
export const effectDescriptions = [
  'Подсветка выключена.',
  'Ровный свет без движения.',
  'Клавиша загорается при нажатии.',
  'Клавиша вспыхивает при отпускании.',
  'Отдельные клавиши мягко мерцают.',
  'Свет движется сверху вниз.',
  'Цвета плавно меняются на разных клавишах.',
  'Яркость плавно нарастает и гаснет.',
  'Вся клавиатура плавно меняет цвет.',
  'Свет расходится от центра.',
  'Цветные полосы перемещаются по клавиатуре.',
  'Радужная волна проходит через клавиши.',
  'Цветовой сектор вращается вокруг центра.',
  'Короткие вспышки света.',
  'Световой импульс проходит по ряду.',
  'Круги расходятся от выбранной клавиши.',
  'Плавный поток света с несколькими волнами.',
  'Свет пульсирует от центра.',
  'Световые полосы движутся по диагонали.',
  'Две волны движутся навстречу друг другу.',
  'Используются индивидуальные цвета клавиш.',
];
export const reactiveModes = [2, 3, 15];
export interface PreviewKey {
  slot: number;
  x: number;
  y: number;
  width: number;
  height: number;
}
export interface LightPulse {
  slot: number;
  at: number;
  x: number;
  y: number;
}
const unit = (n: number) => Math.max(0, Math.min(1, n));
export function effectColor(
  light: LightingSettings,
  key: PreviewKey,
  seconds: number,
  assigned: Rgb,
  pulses: LightPulse[] = [],
): string {
  const x = (key.x + key.width / 2) / 1044,
    y = (key.y + key.height / 2) / 356;
  const time = seconds * (0.15 + light.speed * 0.22) * (light.direction === 1 ? -1 : 1);
  const distance = Math.hypot((x - 0.5) * 2, y - 0.5);
  let hue = x - time * 0.2,
    level = 1;
  switch (light.mode) {
    case 0:
      level = 0;
      break;
    case 1:
      hue = 0.58;
      break;
    case 2:
    case 3:
      level = Math.max(
        0,
        ...pulses.filter((p) => p.slot === key.slot).map((p) => unit(1 - (seconds - p.at) / 1.1)),
      );
      break;
    case 4:
      level = Math.pow(Math.max(0, Math.sin(time * 2 + key.slot * 12.9898)), 12);
      break;
    case 5:
      level = Math.pow(Math.max(0, Math.sin((y - time + x * 3) * 6)), 6);
      break;
    case 6:
      hue = (x + y + time) * 0.3;
      break;
    case 7:
      level = (Math.sin(time * 3) + 1) / 2;
      break;
    case 8:
      hue = time * 0.2;
      break;
    case 9:
      hue = distance - time;
      break;
    case 10:
      hue = Math.floor((x - time * 0.2) * 6) / 6;
      break;
    case 11:
      hue = x - time * 0.2;
      break;
    case 12:
      hue = Math.atan2(y - 0.5, x - 0.5) / (2 * Math.PI) + time * 0.2;
      break;
    case 13:
      level = Math.pow((Math.sin(time * 6) + 1) / 2, 18);
      break;
    case 14:
      level = Math.pow(Math.max(0, Math.sin((x - time * 0.5) * 6)), 8);
      break;
    case 15:
      level = Math.max(
        0,
        ...pulses.map((p) => {
          const age = seconds - p.at;
          const d = Math.hypot((x - p.x) * 2, y - p.y);
          return unit(1 - Math.abs(d - age * 0.8) * 10) * unit(1 - age / 2);
        }),
      );
      break;
    case 16:
      hue = x + Math.sin(y * 3 + time) * 0.2 - time * 0.2;
      break;
    case 17:
      level = (Math.sin(distance * 8 - time * 4) + 1) / 2;
      break;
    case 18:
      hue = x + y * 0.4 - time * 0.3;
      break;
    case 19:
      hue = Math.abs(x - 0.5) * 2 - time * 0.3;
      break;
  }
  const brightness = unit(light.brightness / 5) * level;
  if (light.mode === 20 || light.colorMode === 0) {
    const c = light.mode === 20 ? assigned : light.color;
    return `rgb(${[c.r, c.g, c.b].map((v) => Math.round(v * brightness)).join(', ')})`;
  }
  return `hsl(${(((hue % 1) + 1) % 1) * 360} 85% ${brightness * 55}%)`;
}
