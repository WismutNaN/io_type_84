<script setup lang="ts">
import { computed } from 'vue';
import type { KeyboardSnapshot, MonitorFrame } from '../../shared/contracts/generated';
import { keyboardKeys } from '../../shared/keyboard-view/layout';
import { sampleDepth } from './telemetry';
import { bindingName, hexColor } from './model';
import { t, mm } from '../../shared/ui/preferences';
const props = defineProps<{
  snapshot: KeyboardSnapshot | null;
  selected: number[];
  live: MonitorFrame;
  functionLayer: boolean;
}>();
const key = computed(() => props.snapshot?.keys[props.selected[0] ?? -1]);
const label = computed(() => keyboardKeys.find((k) => k.slot === props.selected[0])?.label ?? '');
const depth = computed(() => sampleDepth(props.live, props.selected[0] ?? -1));
const color = computed(() =>
  props.live.colorAgeMs !== null && props.live.colorAgeMs < 2000
    ? props.live.colors.find((c) => c.ledId === key.value?.ledId)?.color
    : null,
);
</script>
<template>
  <div v-if="key && selected.length === 1" class="key-inspection">
    <div class="inspection-key">
      <strong>{{ label }}</strong
      ><span>{{ t(bindingName(key[functionLayer ? 'function' : 'base'], label)) }}</span>
    </div>
    <div class="inspection-travel">
      <span class="field-heading"
        ><span>{{ t('Глубина нажатия') }}</span
        ><output>{{ depth === null ? '—' : mm(depth) }} {{ t('мм') }}</output></span
      >
      <div
        class="depth-ruler"
        role="meter"
        :aria-label="t('Глубина нажатия')"
        :aria-valuenow="depth ?? 0"
        :aria-valuetext="depth === null ? t('Нет свежих данных') : mm(depth) + ' ' + t('мм')"
        aria-valuemin="0"
        aria-valuemax="3200"
      >
        <span :style="{ width: `${Math.min(depth ?? 0, 3200) / 32}%` }"></span
        ><i
          :style="{ left: `${Math.min(key.actuation.triggerUm, 3200) / 32}%` }"
          :title="t('Точка срабатывания')"
        ></i>
      </div>
      <div class="range-labels">
        <span>0</span><span>3,20 {{ t('мм') }}</span>
      </div>
      <p v-if="!live.active" class="hint">{{ t('Включите наблюдение для измерений') }}</p>
    </div>
    <div class="inspection-colors">
      <span
        ><i :style="{ background: hexColor(key.color) }"></i>{{ t('Заданный цвет')
        }}<b>{{ hexColor(key.color) }}</b></span
      ><span
        ><i
          :class="{ unknown: !color }"
          :style="{ background: color ? hexColor(color) : undefined }"
        ></i
        >{{ t('Текущий цвет') }}<b>{{ color ? hexColor(color) : '—' }}</b></span
      >
    </div>
  </div>
</template>
