<script setup lang="ts">
import { computed } from 'vue';
import { keyboardKeys, keyboardBounds } from './layout';
import type { KeyboardSnapshot, MonitorFrame } from '../contracts/generated';
import { hexColor, bindingName } from '../../features/editor/model';
import { physicalKeyCodes } from './keycodes';
const props = defineProps<{
  selected: number[];
  snapshot: KeyboardSnapshot | null;
  live: MonitorFrame;
  view: 'layout' | 'travel' | 'colors';
  functionLayer: boolean;
  multi: boolean;
  actualColors: boolean;
}>();
const emit = defineEmits<{ select: [slot: number, toggle: boolean, paint?: boolean] }>();
const travel = computed(() => new Map(props.live.travel.map((k) => [k.slot, k])));
const colors = computed(() => new Map(props.live.colors.map((c) => [c.ledId, c.color])));
function depth(slot: number) {
  const k = travel.value.get(slot);
  return props.live.active && k && k.ageMs < 600 ? k.travelUm : null;
}
function color(slot: number) {
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
</script>

<template>
  <div class="keyboard-preview">
    <div class="keyboard-case">
      <div class="case-top">
        <span>io <b>TYPE 84</b></span
        ><span class="panel-marker" title="Геометрия и протокол LED-панели ещё исследуются"
          >LED-панель <i></i><i></i><i></i><i></i><i></i
        ></span>
      </div>
      <div class="keyboard-scroll">
        <div
          class="keyboard-board"
          :style="{ aspectRatio: `${keyboardBounds.width} / ${keyboardBounds.height}` }"
          aria-label="Раскладка IO Type 84"
        >
          <button
            v-for="key in keyboardKeys"
            :key="key.slot"
            class="keycap"
            :class="{
              selected: selected.includes(key.slot),
              pressed: (depth(key.slot) ?? 0) >= 300,
              measured: view === 'travel' && depth(key.slot) !== null,
            }"
            :style="{
              left: `${(key.x / keyboardBounds.width) * 100}%`,
              top: `${(key.y / keyboardBounds.height) * 100}%`,
              width: `${(key.width / keyboardBounds.width) * 100}%`,
              height: `${(key.height / keyboardBounds.height) * 100}%`,
              '--travel': `${Math.min((depth(key.slot) ?? 0) / 3200, 1) * 100}%`,
              '--key-color': color(key.slot) ?? 'transparent',
            }"
            :aria-label="`${key.label}${depth(key.slot) !== null ? `, ход ${((depth(key.slot) ?? 0) / 1000).toFixed(2)} мм` : ''}`"
            :aria-pressed="selected.includes(key.slot)"
            :title="
              bindingName(
                snapshot?.keys[key.slot]?.[functionLayer ? 'function' : 'base'],
                key.label,
              )
            "
            @pointerdown.prevent="
              emit('select', key.slot, $event.ctrlKey || $event.metaKey || multi)
            "
            @pointerenter="$event.buttons === 1 && emit('select', key.slot, true, true)"
            @keydown.space.prevent="emit('select', key.slot, true)"
            @keydown.enter.prevent="emit('select', key.slot, false)"
          >
            <span v-if="view === 'travel'" class="key-measure">{{
              depth(key.slot) === null ? '—' : ((depth(key.slot) ?? 0) / 1000).toFixed(2)
            }}</span>
            <span class="key-label">{{ key.label === 'Space' ? 'Space' : key.label }}</span>
            <span v-if="view === 'layout' && customBinding(key.slot)" class="key-binding">{{
              bindingName(
                snapshot?.keys[key.slot]?.[functionLayer ? 'function' : 'base'],
                key.label,
              )
            }}</span>
            <span
              v-if="view === 'colors'"
              class="key-color"
              :class="{ unknown: !color(key.slot) }"
            ></span>
            <span v-if="view === 'travel'" class="key-travel-fill"></span>
          </button>
        </div>
      </div>
    </div>
    <div class="keyboard-legend">
      <span
        ><i class="legend-selected"></i>Выбор <i class="legend-pressed"></i>Физическое
        движение</span
      ><span v-if="view === 'travel'">{{
        live.active ? 'Ход в мм · — нет свежего измерения' : 'Включите наблюдение для измерений'
      }}</span
      ><span v-else>Ctrl + клик — несколько · протяните для группы</span>
    </div>
  </div>
</template>
