<script setup lang="ts">
import { t, mm } from '../../shared/ui/preferences';
import { computed, ref, watch } from 'vue';
import type { DksConfiguration, Edit, KeyboardSnapshot } from '../../shared/contracts/generated';
import { clone, firmwareActionChoices } from './model';
import TravelSlider from '../../shared/ui/TravelSlider.vue';
const props = defineProps<{
  snapshot: KeyboardSnapshot;
  selected: number[];
  functionLayer: boolean;
  disabled: boolean;
}>();
const emit = defineEmits<{ stage: [...edits: Edit[]] }>();
const noFreeSlot = ref(false);
const form = ref<DksConfiguration>({
  index: 0,
  thresholds: [16, 30, 30, 16],
  actions: [4, 0, 0, 0],
  states: [1, 0, 0, 0],
});
watch(
  () => [props.selected, props.snapshot.dks, props.functionLayer],
  () => {
    const key = props.snapshot.keys[props.selected[0] ?? -1];
    const b = key?.[props.functionLayer ? 'function' : 'base'];
    const existing = b?.page === 8 ? props.snapshot.dks[b.parameters[0]] : undefined;
    noFreeSlot.value = false;
    if (existing) form.value = clone(existing);
    else {
      const free = props.snapshot.dks.find(
        (d) =>
          d.thresholds.every((v) => v === 0) &&
          d.actions.every((v) => v === 0) &&
          d.states.every((v) => v === 0) &&
          !props.snapshot.keys.some((k) =>
            [k.base, k.function].some((b) => b.page === 8 && b.parameters[0] === d.index),
          ),
      );
      noFreeSlot.value = !free;
      form.value = {
        index: free?.index ?? 0,
        thresholds: [16, 30, 30, 16],
        actions: [4, 0, 0, 0],
        states: [1, 0, 0, 0],
      };
    }
  },
  { immediate: true },
);
function state(action: number, phase: number) {
  const mask = form.value.states[phase] ?? 0;
  return mask & (1 << action) ? 1 : mask & (1 << (action + 4)) ? 2 : 0;
}
const phase = ref(0);
const phaseLabels = ['Первое нажатие', 'Глубокое нажатие', 'Начало отпускания', 'Конец отпускания'];
const threshold = computed({
  get: () => (form.value.thresholds[phase.value] ?? 1) * 100,
  set: (value: number) => {
    const n = Math.round(value / 100);
    form.value.thresholds[phase.value] = n;
    if (phase.value === 0) form.value.thresholds[1] = Math.max(n, form.value.thresholds[1]!);
    if (phase.value === 1) form.value.thresholds[0] = Math.min(n, form.value.thresholds[0]!);
    if (phase.value === 2) form.value.thresholds[3] = Math.min(n, form.value.thresholds[3]!);
    if (phase.value === 3) form.value.thresholds[2] = Math.max(n, form.value.thresholds[2]!);
  },
});
function setState(action: number, next: number) {
  let mask = (form.value.states[phase.value] ?? 0) & ~((1 << action) | (1 << (action + 4)));
  if (next) mask |= 1 << (action + (next === 2 ? 4 : 0));
  form.value.states[phase.value] = mask;
}
function stage() {
  if (noFreeSlot.value) return;
  emit(
    'stage',
    { kind: 'dks', value: clone(form.value) },
    {
      kind: 'binding',
      slots: props.selected,
      functionLayer: props.functionLayer,
      binding: { page: 8, parameters: [form.value.index, 0, 0] },
    },
  );
}
</script>
<template>
  <section class="dks-editor">
    <div class="field-heading">
      <h3>{{ t('Действия по ходу') }} · DKS</h3>
      <span class="tag">{{ t('На клавиатуре') }}</span>
    </div>
    <p class="hint">{{ t('Выберите этап движения, задайте глубину и действие.') }}</p>
    <p v-if="noFreeSlot" class="hint">{{ t('Нет свободной записи') }}</p>
    <fieldset :disabled="disabled || selected.length !== 1 || noFreeSlot">
      <div class="phase-tabs">
        <button
          v-for="(label, index) in phaseLabels"
          :key="index"
          :class="{ chosen: phase === index }"
          :aria-pressed="phase === index"
          @click="phase = index"
        >
          <b>{{ index < 2 ? '↓' : '↑' }} {{ mm((form.thresholds[index] ?? 0) * 100) }}</b
          ><span>{{ t(label) }}</span>
        </button>
      </div>
      <TravelSlider v-model="threshold" :label="phaseLabels[phase]!" :step="100" />
      <div class="dks-action-rows">
        <div v-for="(_, index) in form.actions" :key="index">
          <select
            v-model.number="form.actions[index]"
            :aria-label="t('Действие') + ' ' + (index + 1)"
          >
            <option :value="0">{{ t('Не задано') }}</option>
            <option v-for="key in firmwareActionChoices" :key="key.code" :value="key.code">
              {{ key.label }}
            </option>
          </select>
          <div class="segmented">
            <button
              v-for="(label, indexState) in ['Нет', 'Касание', 'Удержание']"
              :key="label"
              :class="{ active: state(index, phase) === indexState }"
              :aria-pressed="state(index, phase) === indexState"
              @click="setState(index, indexState)"
            >
              {{ t(label) }}
            </button>
          </div>
        </div>
      </div>
      <div class="panel-actions">
        <button class="primary" @click="stage">{{ t('Добавить DKS в черновик') }}</button>
      </div>
    </fieldset>
  </section>
</template>
