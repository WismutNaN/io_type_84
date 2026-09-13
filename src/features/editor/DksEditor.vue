<script setup lang="ts">
import { ref, watch } from 'vue';
import type { DksConfiguration, Edit, KeyboardSnapshot } from '../../shared/contracts/generated';
import { clone, keyChoices } from './model';
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
function toggle(action: number, phase: number) {
  const next = (state(action, phase) + 1) % 3;
  let mask = (form.value.states[phase] ?? 0) & ~((1 << action) | (1 << (action + 4)));
  if (next) mask |= 1 << (action + (next === 2 ? 4 : 0));
  form.value.states[phase] = mask;
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
  <section class="surface editor-panel">
    <div class="panel-heading">
      <div>
        <p class="eyebrow">ЧЕТЫРЕ ЭТАПА ХОДА</p>
        <h2>Динамическое нажатие · DKS</h2>
      </div>
      <span class="tag">{{ noFreeSlot ? 'Нет свободной записи' : `Слот ${form.index + 1}` }}</span>
    </div>
    <p class="hint">
      Выберите одну клавишу. Ячейка переключается: ничего → короткое действие → удержание.
    </p>
    <p v-if="noFreeSlot" class="hint">
      Все записи заняты. Можно выбрать клавишу с уже назначенным DKS и изменить её запись.
    </p>
    <fieldset :disabled="disabled || selected.length !== 1 || noFreeSlot">
      <div class="dks-table-wrap">
        <table class="dks-table">
          <thead>
            <tr>
              <th>Действие</th>
              <th
                v-for="(label, phase) in ['↓ Порог 1', '↓ Порог 2', '↑ Порог 1', '↑ Порог 2']"
                :key="label"
              >
                {{ label
                }}<label
                  ><input
                    :value="(form.thresholds[phase] ?? 0) / 10"
                    type="number"
                    min="0.1"
                    max="3.2"
                    step="0.1"
                    :aria-label="label"
                    @change="
                      form.thresholds[phase] = Math.round(
                        Number(($event.target as HTMLInputElement).value) * 10,
                      )
                    "
                  />
                  мм</label
                >
              </th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(_, action) in form.actions" :key="action">
              <td>
                <select
                  v-model.number="form.actions[action]"
                  :aria-label="'Действие ' + (action + 1)"
                >
                  <option :value="0">Не задано</option>
                  <option v-for="k in keyChoices" :key="k.code" :value="k.code">
                    {{ k.label }}
                  </option>
                </select>
              </td>
              <td v-for="phase in [0, 1, 2, 3]" :key="phase">
                <button
                  class="dks-state"
                  :class="{ tap: state(action, phase) === 1, hold: state(action, phase) === 2 }"
                  @click="toggle(action, phase)"
                >
                  {{ ['—', 'Касание', 'Удержание'][state(action, phase)] }}
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <div class="panel-actions">
        <span class="hint">Формат SDK; аппаратное поведение ещё проверяется.</span
        ><button class="primary" @click="stage">Добавить DKS в черновик</button>
      </div>
    </fieldset>
  </section>
</template>
