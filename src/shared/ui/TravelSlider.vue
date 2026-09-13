<script setup lang="ts">
import { t, mm } from './preferences';
const value = defineModel<number>({ required: true });
withDefaults(
  defineProps<{
    label: string;
    min?: number;
    max?: number;
    step?: number;
    live?: number | null;
    low?: string;
    high?: string;
  }>(),
  { min: 100, max: 3200, step: 10, live: null, low: 'Легче', high: 'Глубже' },
);
</script>
<template>
  <label class="travel-slider"
    ><span class="field-heading"
      ><span>{{ t(label) }}</span
      ><output>{{ mm(value) }} {{ t('мм') }}</output></span
    ><span class="range-visual"
      ><input
        v-model.number="value"
        type="range"
        :min="min"
        :max="max"
        :step="step"
        :aria-label="t(label)"
        :aria-valuetext="`${mm(value)} ${t('мм')}`"
        :style="{ '--range': `${((value - min) / (max - min)) * 100}%` }" /><i
        v-if="live !== null"
        class="live-caret"
        :style="{ left: `${Math.max(0, Math.min(100, ((live - min) / (max - min)) * 100))}%` }"
        :title="`${mm(live)} ${t('мм')}`"
      ></i></span
    ><span class="range-labels"
      ><span>{{ t(low) }}</span
      ><span>{{ t(high) }}</span></span
    ></label
  >
</template>
