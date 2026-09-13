<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type { ComputerAction, DepthRule, MonitorFrame } from '../../shared/contracts/generated';
import { computerActions } from './computer-rules';
import { sampleDepth } from './telemetry';
import { t, mm } from '../../shared/ui/preferences';
import TravelSlider from '../../shared/ui/TravelSlider.vue';
const props = defineProps<{
  selected: number[];
  rules: DepthRule[];
  live: MonitorFrame;
  disabled: boolean;
}>();
const emit = defineEmits<{ save: [rules: DepthRule[]] }>();
const threshold = ref(2400),
  action = ref<ComputerAction>('volumeUp');
const current = computed(() => props.rules.find((r) => r.slot === props.selected[0]));
watch(
  [() => props.selected, current],
  () => {
    threshold.value = current.value?.thresholdUm ?? 2400;
    action.value = current.value?.action ?? (props.selected[0] === 108 ? 'volumeDown' : 'volumeUp');
  },
  { immediate: true },
);
const depth = computed(() => sampleDepth(props.live, props.selected[0] ?? -1));
function save() {
  if (props.selected.length !== 1) return;
  emit('save', [
    ...props.rules.filter((r) => r.slot !== props.selected[0]),
    {
      slot: props.selected[0]!,
      thresholdUm: threshold.value,
      releaseUm: Math.max(0, threshold.value - 600),
      action: action.value,
    },
  ]);
}
</script>
<template>
  <section class="depth-actions">
    <div class="field-heading">
      <h3>{{ t('Глубокое нажатие') }}</h3>
      <span class="tag">{{ t('На компьютере') }}</span>
    </div>
    <p class="hint">
      {{
        t('Дополнительное действие при пересечении порога. Обычное назначение клавиши сохраняется.')
      }}
    </p>
    <fieldset :disabled="disabled || selected.length !== 1">
      <div class="action-tiles">
        <button
          v-for="a in computerActions"
          :key="a.id"
          :class="{ chosen: action === a.id }"
          :aria-pressed="action === a.id"
          @click="action = a.id"
        >
          <b>{{ a.short }}</b
          ><span>{{ t(a.label) }}</span>
        </button>
      </div>
      <TravelSlider v-model="threshold" label="Сработает на глубине" :min="300" :live="depth" />
      <div class="condition-preview">
        <span
          >{{ t('Нажатие') }} <b>{{ mm(threshold) }} {{ t('мм') }}</b></span
        ><span>→</span><b>{{ t(computerActions.find((a) => a.id === action)?.label ?? '') }}</b>
      </div>
      <p class="hint">
        {{ t('Один раз. Повтор — после возврата выше отметки') }}
        {{ mm(Math.max(0, threshold - 600)) }} {{ t('мм') }}.
      </p>
      <div class="panel-actions">
        <button
          v-if="current"
          class="text-button"
          @click="
            emit(
              'save',
              rules.filter((r) => r.slot !== selected[0]),
            )
          "
        >
          {{ t('Удалить') }}</button
        ><button class="primary" @click="save">{{ t('Сохранить действие') }}</button>
      </div>
    </fieldset>
  </section>
</template>
