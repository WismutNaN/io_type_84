<script setup lang="ts">
import { computed, ref, onMounted, onBeforeUnmount } from 'vue';
import type { MonitorFrame } from '../../shared/contracts/generated';
import { sampleDepth } from './telemetry';
import { keyboardKeys } from '../../shared/keyboard-view/layout';
import { t, mm } from '../../shared/ui/preferences';
const props = defineProps<{ live: MonitorFrame; selected: number[] }>();
const emit = defineEmits<{ select: [slot: number]; clear: []; capacity: [count: number] }>();
const historyElement = ref<HTMLElement | null>(null);
const visibleCount = ref(1);
const visibleHistory = computed(() => props.live.history.slice(0, visibleCount.value));
let observer: ResizeObserver | undefined;
onMounted(() => {
  observer = new ResizeObserver(([entry]) => {
    visibleCount.value = Math.max(0, Math.floor(((entry?.contentRect.width ?? 0) + 5) / 89));
    emit('capacity', visibleCount.value * 3);
  });
  if (historyElement.value) observer.observe(historyElement.value);
});
onBeforeUnmount(() => observer?.disconnect());
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
    <div ref="historyElement" class="press-history" :aria-label="t('Последние нажатия')">
      <button
        v-for="event in visibleHistory"
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
