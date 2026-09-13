<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import type {
  Edit,
  HardwareMacro,
  KeyboardSnapshot,
  MacroStep,
} from '../../shared/contracts/generated';
import { clone, keyChoices, keyName } from './model';
import { eventKeyCodes } from '../../shared/keyboard-view/keycodes';
const props = defineProps<{ snapshot: KeyboardSnapshot; disabled: boolean }>();
const emit = defineEmits<{ stage: [edit: Edit] }>();
const selected = ref(0),
  steps = ref<MacroStep[]>([]),
  recording = ref(false),
  keepDelays = ref(true),
  defaultDelay = ref(10),
  addCode = ref(4),
  addMouse = ref(false);
const held = new Set<number>();
watch(addMouse, () => {
  addCode.value = addMouse.value ? 1 : 4;
});
let previous = 0;
watch(
  () => props.snapshot.macros,
  (values) => {
    steps.value = clone(values.find((m) => m.id === selected.value)?.steps ?? []);
  },
  { immediate: true },
);
function select(id: number) {
  stop();
  selected.value = id;
  steps.value = clone(props.snapshot.macros.find((m) => m.id === id)?.steps ?? []);
}
const used = computed(
  () =>
    400 +
    props.snapshot.macros
      .filter((m) => m.id !== selected.value && m.steps.length)
      .reduce((n, m) => n + 4 + m.steps.length * 4, 0) +
    (steps.value.length ? 4 + steps.value.length * 4 : 0),
);
function record(event: KeyboardEvent, pressed: boolean) {
  if (!recording.value) return;
  if (event.code === 'Escape') {
    event.preventDefault();
    stop();
    return;
  }
  const code = eventKeyCodes[event.code];
  if (code === undefined || code === 175) return;
  event.preventDefault();
  event.stopPropagation();
  if (pressed && event.repeat) return;
  if (pressed) held.add(code);
  else if (!held.delete(code)) return;
  const now = performance.now();
  steps.value.push({
    kind: 1,
    keyCode: code,
    pressed,
    delayMs:
      keepDelays.value && previous
        ? Math.min(65535, Math.round(now - previous))
        : defaultDelay.value,
  });
  previous = now;
  if (steps.value.length >= 26) stop();
}
const down = (e: KeyboardEvent) => record(e, true),
  up = (e: KeyboardEvent) => record(e, false);
function start() {
  recording.value = true;
  previous = 0;
  held.clear();
  window.addEventListener('keydown', down, true);
  window.addEventListener('keyup', up, true);
  window.addEventListener('blur', stop);
}
function stop() {
  for (const code of held) steps.value.push({ kind: 1, keyCode: code, pressed: false, delayMs: 0 });
  held.clear();
  recording.value = false;
  window.removeEventListener('keydown', down, true);
  window.removeEventListener('keyup', up, true);
  window.removeEventListener('blur', stop);
}
onBeforeUnmount(stop);
function stage(remove = false) {
  stop();
  const values: HardwareMacro[] = props.snapshot.macros.filter((m) => m.id !== selected.value);
  if (!remove && steps.value.length) values.push({ id: selected.value, steps: clone(steps.value) });
  emit('stage', { kind: 'macros', values: values.sort((a, b) => a.id - b.id) });
}
function addPair() {
  steps.value.push(
    {
      kind: addMouse.value ? 3 : 1,
      keyCode: addCode.value,
      pressed: true,
      delayMs: defaultDelay.value,
    },
    {
      kind: addMouse.value ? 3 : 1,
      keyCode: addCode.value,
      pressed: false,
      delayMs: defaultDelay.value,
    },
  );
}
function move(index: number, offset: number) {
  const target = index + offset;
  if (target < 0 || target >= steps.value.length) return;
  const [step] = steps.value.splice(index, 1);
  if (step) steps.value.splice(target, 0, step);
}
</script>

<template>
  <section class="surface editor-panel">
    <div class="panel-heading">
      <div>
        <p class="eyebrow">ПОСЛЕДОВАТЕЛЬНОСТИ</p>
        <h2>Макросы без лишних движений</h2>
      </div>
      <span class="tag" :class="{ danger: used > 512 }">{{ used }} / 512 байт</span>
    </div>
    <p class="hint">
      512 байт — временный консервативный лимит вместе с каталогом. Большая ёмкость прошивки ещё
      исследуется.
    </p>
    <div class="macro-workspace">
      <div class="macro-list">
        <button
          v-for="m in snapshot.macros"
          :key="m.id"
          :class="{ active: selected === m.id }"
          @click="select(m.id)"
        >
          <b>Макрос {{ m.id + 1 }}</b
          ><small>{{ m.steps.length }} шагов</small></button
        ><button
          class="secondary"
          @click="
            select(
              Array.from({ length: 100 }, (_, i) => i).find(
                (i) => !snapshot.macros.some((m) => m.id === i),
              ) ?? 0,
            )
          "
        >
          ＋ Новый макрос
        </button>
      </div>
      <div class="macro-steps">
        <div class="inline-toolbar">
          <h3>Макрос {{ selected + 1 }}</h3>
          <button
            class="record-button"
            :class="{ recording }"
            :disabled="disabled"
            @click="recording ? stop() : start()"
          >
            {{ recording ? '■ Остановить · Esc' : '● Записать в этом окне' }}
          </button>
        </div>
        <p v-if="recording" class="recording-hint" role="status">
          Запись активна. Клавиши перехватываются только в этом окне. Потеря фокуса завершит запись.
        </p>
        <div class="macro-options">
          <label class="check-line"
            ><input v-model="keepDelays" type="checkbox" />Записывать интервалы</label
          ><label
            >Интервал, мс <input v-model.number="defaultDelay" type="number" min="0" max="65535"
          /></label>
        </div>
        <div class="step-table-wrap">
          <table class="step-table">
            <thead>
              <tr>
                <th>№</th>
                <th>Клавиша</th>
                <th>Действие</th>
                <th>Пауза, мс</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(step, index) in steps" :key="index">
                <td>{{ index + 1 }}</td>
                <td>{{ step.kind === 3 ? 'Мышь ' + step.keyCode : keyName(step.keyCode) }}</td>
                <td>
                  <select v-model="step.pressed" aria-label="Действие шага">
                    <option :value="true">↓ Нажать</option>
                    <option :value="false">↑ Отпустить</option>
                  </select>
                </td>
                <td>
                  <input
                    v-model.number="step.delayMs"
                    type="number"
                    min="0"
                    max="65535"
                    aria-label="Задержка шага"
                  />
                </td>
                <td class="step-controls">
                  <button :disabled="index === 0" title="Выше" @click="move(index, -1)">↑</button
                  ><button
                    :disabled="index === steps.length - 1"
                    title="Ниже"
                    @click="move(index, 1)"
                  >
                    ↓</button
                  ><button title="Удалить шаг" @click="steps.splice(index, 1)">×</button>
                </td>
              </tr>
            </tbody>
          </table>
          <p v-if="!steps.length" class="empty-inline">Добавьте действие или начните запись.</p>
        </div>
        <div class="add-step">
          <label class="check-line"
            ><input
              v-model="addMouse"
              type="checkbox"
              @change="addCode = addMouse ? 1 : 4"
            />Мышь</label
          ><select v-model.number="addCode" aria-label="Добавляемая клавиша">
            <template v-if="!addMouse"
              ><option v-for="k in keyChoices" :key="k.code" :value="k.code">
                {{ k.label }}
              </option></template
            ><template v-else
              ><option :value="1">Левая</option>
              <option :value="2">Правая</option>
              <option :value="4">Средняя</option>
              <option :value="8">Назад</option>
              <option :value="16">Вперёд</option></template
            ></select
          ><button class="secondary" @click="addPair">＋ Нажать и отпустить</button>
        </div>
        <div class="panel-actions">
          <button class="text-button" :disabled="disabled" @click="stage(true)">
            Удалить макрос</button
          ><button class="primary" :disabled="disabled || used > 512 || recording" @click="stage()">
            Сохранить в черновик
          </button>
        </div>
      </div>
    </div>
  </section>
</template>
