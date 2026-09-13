<script setup lang="ts">
import { computed } from 'vue';
import { t, mm } from '../ui/preferences';
import { actionLabel as catalogLabel } from '../../features/editor/computer-rules';
import type { AutomationProfile } from '../contracts/generated';
import { keyboardKeys, keyboardBounds } from './layout';
import type { KeyboardSnapshot, MonitorFrame } from '../contracts/generated';
import { hexColor, bindingName } from '../../features/editor/model';
import { physicalKeyCodes } from './keycodes';
const props = defineProps<{
  selected: number[];
  automation?: AutomationProfile;
  snapshot: KeyboardSnapshot | null;
  live: MonitorFrame;
  view: 'layout' | 'travel' | 'colors';
  functionLayer: boolean;
  multi: boolean;
  actualColors: boolean;
  previewColors?: Record<number, string>;
}>();
const emit = defineEmits<{ select: [slot: number, toggle: boolean, paint?: boolean] }>();
const travel = computed(() => new Map(props.live.travel.map((k) => [k.slot, k])));
const colors = computed(() => new Map(props.live.colors.map((c) => [c.ledId, c.color])));
function depth(slot: number) {
  const k = travel.value.get(slot);
  return props.live.active && k && k.ageMs < 600 ? k.travelUm : null;
}
function color(slot: number) {
  if (props.previewColors) return props.previewColors[slot] ?? null;
  const k = props.snapshot?.keys[slot];
  if (!k) return null;
  if (props.actualColors) {
    const rgb = colors.value.get(k.ledId);
    return rgb && props.live.colorAgeMs !== null && props.live.colorAgeMs < 2000
      ? hexColor(rgb)
      : null;
  }
  return hexColor(k.color);
}
function customBinding(slot: number) {
  const b = props.snapshot?.keys[slot]?.[props.functionLayer ? 'function' : 'base'];
  return (
    b &&
    b.page !== 0 &&
    !(b.page === 2 && b.parameters[0] === 0 && b.parameters[1] === physicalKeyCodes[slot])
  );
}

function actionLabel(slot: number, original: string) {
  const b = props.snapshot?.keys[slot]?.[props.functionLayer ? 'function' : 'base'];
  if (!customBinding(slot)) return original;
  const name = t(bindingName(b, original));
  return (
    (
      {
        'Page Up': 'PgUp',
        'Page Down': 'PgDn',
        'Print Screen': 'Print',
        'Scroll Lock': 'Scroll',
        'Caps Lock': 'Caps',
      } as Record<string, string>
    )[name] ?? name
  );
}
function deepLabel(slot: number) {
  const matches = props.automation?.gestures.filter((r) => r.slots.includes(slot)) ?? [];
  const first = matches[0];
  return first
    ? catalogLabel(props.automation?.actions.find((a) => a.id === first.actionId)) +
        (matches.length > 1 ? ` +${matches.length - 1}` : '')
    : null;
}
function navigate(event: KeyboardEvent, slot: number) {
  const key = keyboardKeys.find((k) => k.slot === slot)!;
  const direction = event.key;
  if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(direction)) return;
  event.preventDefault();
  const candidates = keyboardKeys.filter((k) =>
    direction === 'ArrowLeft'
      ? k.x < key.x && k.y === key.y
      : direction === 'ArrowRight'
        ? k.x > key.x && k.y === key.y
        : direction === 'ArrowUp'
          ? k.y < key.y
          : k.y > key.y,
  );
  candidates.sort(
    (a, b) =>
      Math.abs(a.y - key.y) * 10 +
      Math.abs(a.x - key.x) -
      Math.abs(b.y - key.y) * 10 -
      Math.abs(b.x - key.x),
  );
  if (candidates[0]) {
    const next = candidates[0].slot;
    (event.currentTarget as HTMLElement).parentElement
      ?.querySelector<HTMLButtonElement>(`[data-slot="${next}"]`)
      ?.focus();
  }
}
</script>
<template>
  <div class="keyboard-preview">
    <div class="keyboard-case">
      <div class="keyboard-scroll">
        <div
          class="keyboard-board"
          :style="{ aspectRatio: `${keyboardBounds.width} / ${keyboardBounds.height}` }"
          :aria-label="t('Раскладка IO Type 84')"
        >
          <div
            class="led-panel"
            :title="t('LED-панель · прямое управление исследуется')"
            role="img"
            :aria-label="t('LED-панель')"
          >
            <span v-for="n in 12" :key="n"></span>
          </div>
          <button
            v-for="key in keyboardKeys"
            :key="key.slot"
            :data-slot="key.slot"
            class="keycap"
            :class="{
              selected: selected.includes(key.slot),
              pressed: (depth(key.slot) ?? 0) >= 300,
              custom: customBinding(key.slot),
              'has-depth-action': deepLabel(key.slot),
              'light-preview': !!previewColors,
            }"
            :style="{
              left: `${(key.x / keyboardBounds.width) * 100}%`,
              top: `${(key.y / keyboardBounds.height) * 100}%`,
              width: `${(key.width / keyboardBounds.width) * 100}%`,
              height: `${(key.height / keyboardBounds.height) * 100}%`,
              '--travel': `${Math.min((depth(key.slot) ?? 0) / 3200, 1) * 100}%`,
              '--key-color': color(key.slot) ?? 'transparent',
            }"
            :aria-pressed="selected.includes(key.slot)"
            :aria-label="`${key.label}: ${actionLabel(key.slot, key.label)}${deepLabel(key.slot) ? ', ' + t('Глубокое нажатие') + ': ' + deepLabel(key.slot) : ''}`"
            :title="`${actionLabel(key.slot, key.label)}${depth(key.slot) !== null ? ' · ' + mm(depth(key.slot)!) + ' ' + t('мм') : ''}`"
            @pointerdown.prevent="
              emit('select', key.slot, $event.ctrlKey || $event.metaKey || multi)
            "
            @pointerenter="$event.buttons === 1 && emit('select', key.slot, true, true)"
            @keydown.space.prevent="emit('select', key.slot, true)"
            @keydown.enter.prevent="emit('select', key.slot, false)"
            @keydown="navigate($event, key.slot)"
          >
            <span class="physical-legend" v-if="customBinding(key.slot)">{{ key.label }}</span>
            <span class="key-label">{{ actionLabel(key.slot, key.label) }}</span>
            <span v-if="deepLabel(key.slot)" class="key-binding">↓ {{ deepLabel(key.slot) }}</span>
            <span v-if="view === 'travel' && !deepLabel(key.slot)" class="key-measure">{{
              depth(key.slot) === null ? '—' : mm(depth(key.slot)!)
            }}</span>
            <span
              v-if="view === 'colors'"
              class="key-color"
              :class="{ unknown: !color(key.slot) }"
            ></span>
            <span v-if="live.active" class="key-travel-track"
              ><span class="key-travel-fill"></span
            ></span>
          </button>
        </div>
      </div>
    </div>
    <div class="keyboard-legend">
      <span
        ><i class="legend-selected"></i>{{ t('Выбрано') }}<i class="legend-pressed"></i
        >{{ t('Глубина нажатия') }}</span
      ><span>{{ t('Ctrl + клик — группа · стрелки — перемещение') }}</span>
    </div>
  </div>
</template>
