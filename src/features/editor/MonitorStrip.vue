<script setup lang="ts">
import { computed } from 'vue';
import type { MonitorFrame } from '../../shared/contracts/generated';
import { sampleDepth } from './telemetry';
import { keyboardKeys } from '../../shared/keyboard-view/layout';
import { t, mm } from '../../shared/ui/preferences';
const props = defineProps<{ live: MonitorFrame; selected: number[] }>();
defineEmits<{ select: [slot: number]; clear: [] }>();
const label = (slot: number) => keyboardKeys.find((k) => k.slot === slot)?.label ?? String(slot);
const depth = computed(() => sampleDepth(props.live, props.selected[0] ?? -1));
</script>
<template>
  <div class="monitor-strip">
    <div class="current-travel">
      <span>{{ selected.length === 1 ? label(selected[0]!) : t('Ход') }}</span
      ><meter
        min="0"
        max="3200"
        :value="depth ?? 0"
        :aria-label="t('Глубина нажатия')"
        :aria-valuetext="depth === null ? t('Нет свежих данных') : mm(depth) + ' ' + t('мм')"
      ></meter
      ><output
        >{{ depth === null ? '—' : mm(depth) }} <small>{{ t('мм') }}</small></output
      >
    </div>
    <div class="press-history" :aria-label="t('Последние 20 нажатий')">
      <button
        v-for="event in live.history"
        :key="event.sequence"
        :title="`${label(event.slot)} · ${mm(event.peakUm)} ${t('мм')}`"
        @click="$emit('select', event.slot)"
      >
        <b>{{ label(event.slot) }}</b
        ><small>{{ mm(event.peakUm) }}</small></button
      ><span v-if="!live.history.length" class="hint">{{
        t('Здесь появятся последние нажатия')
      }}</span>
    </div>
    <button class="text-button" :disabled="!live.history.length" @click="$emit('clear')">
      {{ t('Очистить') }}
    </button>
  </div>
</template>
