<script setup lang="ts">
import { t } from '../../shared/ui/preferences';
import { computed, ref, watch } from 'vue';
import TravelSlider from '../../shared/ui/TravelSlider.vue';
import { sampleDepth } from './telemetry';
import type { MonitorFrame } from '../../shared/contracts/generated';
import type { Actuation, Edit, KeyboardSnapshot } from '../../shared/contracts/generated';
import {
  bindingName,
  clone,
  deviceActions,
  hexColor,
  keyChoices,
  firmwareActionChoices,
  keyName,
  parseColor,
} from './model';
import { physicalKeyCodes } from '../../shared/keyboard-view/keycodes';
import { keyboardKeys } from '../../shared/keyboard-view/layout';
const props = defineProps<{
  snapshot: KeyboardSnapshot | null;
  selected: number[];
  section: string;
  functionLayer: boolean;
  disabled: boolean;
  live: MonitorFrame;
}>();
const emit = defineEmits<{ stage: [...edits: Edit[]] }>();
const key = computed(() => props.snapshot?.keys[props.selected[0] ?? -1]);
const selectionTitle = computed(() =>
  props.selected.length === 1
    ? (keyboardKeys.find((k) => k.slot === props.selected[0])?.label ?? 'Клавиша')
    : `${props.selected.length} клавиш`,
);
const search = ref(''),
  kind = ref('keyboard'),
  code = ref(4),
  macroId = ref(0),
  repeats = ref(1),
  modifier1 = ref(224),
  modifier2 = ref(0),
  advanced = ref('mt'),
  holdCode = ref(225),
  tapCode = ref(4),
  delay = ref(200),
  socd = ref(3);
const color = ref('#a78bfa');
watch(
  kind,
  (value) => {
    if (value === 'mouse' || value === 'wheel') code.value = 1;
    else if (value === 'media') code.value = 0xe9;
    else if (value === 'device') code.value = 11;
    else if (value === 'keyboard' || value === 'combo')
      code.value = physicalKeyCodes[props.selected[0] ?? -1] ?? 4;
    else if (value === 'macro') macroId.value = props.snapshot?.macros[0]?.id ?? 0;
  },
  { flush: 'sync' },
);
const rt = ref<Actuation>({
  triggerUm: 1200,
  pressUm: 100,
  releaseUm: 100,
  rapidTrigger: false,
  wholeTravel: false,
  rampage: false,
  axisType: 0,
});
watch(
  [key, () => props.functionLayer],
  ([k]) => {
    if (k) {
      rt.value = clone(k.actuation);
      if (!rt.value.pressUm) rt.value.pressUm = 100;
      if (!rt.value.releaseUm) rt.value.releaseUm = 100;
      color.value = hexColor(k.color);
      const bound = k[props.functionLayer ? 'function' : 'base'];
      kind.value =
        (
          {
            0: 'default',
            1: 'mouse',
            2: 'keyboard',
            3: 'media',
            6: 'macro',
            7: 'combo',
            13: 'device',
          } as Record<number, string>
        )[bound.page] ?? 'keyboard';
      if (bound.page === 1 && bound.parameters[0] === 3) kind.value = 'wheel';
      code.value =
        bound.page === 3
          ? bound.parameters[0] + bound.parameters[1] * 256
          : bound.page === 13
            ? bound.parameters[2]
            : bound.page === 1
              ? bound.parameters[1]
              : bound.page === 7
                ? bound.parameters[2]
                : bound.page === 2
                  ? bound.parameters[1]
                  : (physicalKeyCodes[k.slot] ?? 4);
      if (bound.page === 6) {
        macroId.value = bound.parameters[0];
        repeats.value = bound.parameters[2] || 1;
      }
      if (bound.page === 7) {
        modifier1.value = bound.parameters[0];
        modifier2.value = bound.parameters[1];
      }
      advanced.value =
        ({ 9: 'mt', 10: 'tgl', 11: 'socd', 12: 'rs' } as Record<number, string>)[bound.page] ??
        'mt';
      if (bound.page === 9) {
        holdCode.value = bound.parameters[0];
        tapCode.value = bound.parameters[1];
        delay.value = bound.parameters[2] * 10;
      }
      if (bound.page === 10) tapCode.value = bound.parameters[0];
      if (bound.page === 11) socd.value = bound.parameters[0];
    }
  },
  { immediate: true },
);
const choices = computed(() =>
  keyChoices.filter((c) =>
    `${c.label} ${c.group}`.toLowerCase().includes(search.value.toLowerCase()),
  ),
);
function bind() {
  let page = 2,
    parameters: [number, number, number] = [0, code.value, 0];
  if (kind.value === 'default') {
    page = 0;
    parameters = [0, 0, 0];
  }
  if (kind.value === 'mouse') {
    page = 1;
    parameters = [1, code.value, 0];
  }
  if (kind.value === 'wheel') {
    page = 1;
    parameters = [3, code.value, 0];
  }
  if (kind.value === 'media') {
    page = 3;
    parameters = [code.value & 255, (code.value >> 8) & 255, 0];
  }
  if (kind.value === 'macro') {
    page = 6;
    parameters = [macroId.value, 1, repeats.value];
  }
  if (kind.value === 'combo') {
    page = 7;
    parameters = [modifier1.value, modifier2.value, code.value];
  }
  if (kind.value === 'device') {
    page = 13;
    parameters = [0, 0, code.value];
  }
  emit('stage', {
    kind: 'binding',
    slots: props.selected,
    functionLayer: props.functionLayer,
    binding: { page, parameters },
  });
}
function advancedEdit() {
  let page = 9,
    parameters: [number, number, number] = [
      holdCode.value,
      tapCode.value,
      Math.round(delay.value / 10),
    ];
  if (advanced.value === 'tgl') {
    page = 10;
    parameters = [tapCode.value, 0, 0];
  }
  if (advanced.value === 'rs' || advanced.value === 'socd') {
    page = advanced.value === 'rs' ? 12 : 11;
    parameters = [
      page === 12 ? 0 : socd.value,
      physicalKeyCodes[props.selected[0] ?? -1] ?? 0,
      physicalKeyCodes[props.selected[1] ?? -1] ?? 0,
    ];
  }
  emit('stage', {
    kind: 'binding',
    slots: props.selected,
    functionLayer: props.functionLayer,
    binding: { page, parameters },
  });
}
</script>

<template>
  <aside class="inspector surface">
    <div v-if="!selected.length" class="selection-empty">
      <h3>{{ t('Выберите клавишу') }}</h3>
      <p>{{ t('Нажмите на макет. Ctrl + клик добавляет клавиши в группу.') }}</p>
    </div>
    <fieldset v-else :disabled="disabled || !snapshot" class="inspector-fields">
      <div class="field-heading" v-if="section !== 'lighting'">
        <h3>{{ selectionTitle }}</h3>
        <span class="tag">{{ t(String(functionLayer ? 'Fn' : 'На клавиатуре')) }}</span>
      </div>
      <p v-if="selected.length > 1" class="hint">
        {{
          t(
            'Показаны значения первой выбранной клавиши. Добавленное изменение применяется ко всей группе.',
          )
        }}
      </p>
      <template v-if="section === 'actuation' || section === 'overview'">
        <TravelSlider
          v-model="rt.triggerUm"
          :label="t('Точка срабатывания')"
          :live="sampleDepth(live, selected[0] ?? -1)"
        />
        <div class="preset-row">
          <button @click="rt.triggerUm = 400">{{ t('Лёгкое') }}</button
          ><button @click="rt.triggerUm = 1200">{{ t('Обычное') }}</button
          ><button @click="rt.triggerUm = 2400">{{ t('Глубокое') }}</button>
        </div>
        <p v-if="rt.triggerUm < 100" class="hint">
          {{ t('В профиле нулевой порог. Выберите точку срабатывания.') }}
        </p>
        <label class="switch-line"
          ><span
            ><b>Rapid Trigger</b><small>{{ t('Повторное срабатывание по движению') }}</small></span
          ><input v-model="rt.rapidTrigger" type="checkbox" role="switch"
        /></label>
        <div v-if="rt.rapidTrigger" class="rt-settings">
          <TravelSlider
            v-model="rt.pressUm"
            :label="t('Повторное нажатие')"
            :min="10"
            :low="t('Чувствительнее')"
            :high="t('Стабильнее')"
          />
          <TravelSlider
            v-model="rt.releaseUm"
            :label="t('Отпускание')"
            :min="10"
            :low="t('Чувствительнее')"
            :high="t('Стабильнее')"
          />
          <details>
            <summary>{{ t('Дополнительно') }}</summary>
            <label class="check-line"
              ><input v-model="rt.wholeTravel" type="checkbox" />{{ t('На всём ходе') }}</label
            ><label class="check-line"><input v-model="rt.rampage" type="checkbox" />Rampage</label>
          </details>
        </div>
        <button
          class="primary full"
          @click="emit('stage', { kind: 'actuation', slots: selected, value: clone(rt) })"
        >
          {{ t('Добавить в черновик') }}
        </button>
      </template>
      <template v-else-if="section === 'keys'">
        <details class="assignment-details">
          <summary>
            {{ t('Основное действие')
            }}<b>{{
              t(bindingName(key?.[functionLayer ? 'function' : 'base'], selectionTitle))
            }}</b>
          </summary>
          <div class="assignment-fields">
            <label
              >{{ t('Тип действия')
              }}<select v-model="kind">
                <option value="keyboard">{{ t('Клавиша') }}</option>
                <option value="media">{{ t('Медиа') }}</option>
                <option value="device">{{ t('Действие клавиатуры / панели') }}</option>
                <option value="mouse">{{ t('Кнопка мыши') }}</option>
                <option value="wheel">{{ t('Прокрутка мыши') }}</option>
                <option value="combo">{{ t('Сочетание клавиш') }}</option>
                <option value="macro">{{ t('Аппаратный макрос') }}</option>
                <option value="default">{{ t('Заводское назначение') }}</option>
              </select></label
            >
            <template v-if="kind === 'keyboard'"
              ><input
                v-model="search"
                class="search"
                type="search"
                :placeholder="t('Найти клавишу…')"
                :aria-label="t('Поиск назначения')"
              />
              <div class="character-grid">
                <button
                  v-for="choice in choices"
                  :key="choice.code"
                  :class="{ chosen: code === choice.code }"
                  :title="t(choice.group)"
                  @click="code = choice.code"
                >
                  {{ choice.label }}
                </button>
              </div>
              <p>
                {{ t('Назначение:') }}<b>{{ keyName(code) }}</b>
              </p></template
            >
            <select v-if="kind === 'media'" v-model.number="code" :aria-label="t('Медиа-действие')">
              <option :value="0xe9">{{ t('Громче') }}</option>
              <option :value="0xea">{{ t('Тише') }}</option>
              <option :value="0xe2">{{ t('Без звука') }}</option>
              <option :value="0xcd">{{ t('Воспроизведение / пауза') }}</option>
              <option :value="0xb5">{{ t('Следующий трек') }}</option>
              <option :value="0xb6">{{ t('Предыдущий трек') }}</option>
              <option :value="0x192">{{ t('Калькулятор') }}</option>
            </select>
            <select
              v-if="kind === 'device'"
              v-model.number="code"
              :aria-label="t('Действие прошивки')"
            >
              <option v-for="a in deviceActions" :key="a.code" :value="a.code">
                {{ t(a.label) }}
              </option>
            </select>
            <select v-if="kind === 'mouse'" v-model.number="code" :aria-label="t('Кнопка мыши')">
              <option :value="1">{{ t('Левая') }}</option>
              <option :value="2">{{ t('Правая') }}</option>
              <option :value="4">{{ t('Средняя') }}</option>
              <option :value="8">{{ t('Назад') }}</option>
              <option :value="16">{{ t('Вперёд') }}</option>
            </select>
            <select
              v-if="kind === 'wheel'"
              v-model.number="code"
              :aria-label="t('Направление прокрутки')"
            >
              <option :value="1">{{ t('Вверх') }}</option>
              <option :value="255">{{ t('Вниз') }}</option>
            </select>
            <template v-if="kind === 'combo'">
              <label
                >{{ t('Модификатор')
                }}<select v-model.number="modifier1">
                  <option
                    v-for="k in keyChoices.filter((k) => k.code >= 224)"
                    :key="k.code"
                    :value="k.code"
                  >
                    {{ k.label }}
                  </option>
                </select></label
              >
              <label
                >{{ t('Второй модификатор')
                }}<select v-model.number="modifier2">
                  <option :value="0">{{ t('Нет') }}</option>
                  <option
                    v-for="k in keyChoices.filter((k) => k.code >= 224 && k.code !== modifier1)"
                    :key="k.code"
                    :value="k.code"
                  >
                    {{ k.label }}
                  </option>
                </select></label
              >
              <label
                >{{ t('Клавиша')
                }}<select v-model.number="code">
                  <option
                    v-for="k in keyChoices.filter((k) => k.code < 116)"
                    :key="k.code"
                    :value="k.code"
                  >
                    {{ k.label }}
                  </option>
                </select></label
              >
            </template>
            <select
              v-if="kind === 'macro'"
              v-model.number="macroId"
              :aria-label="t('Аппаратный макрос')"
            >
              <option v-for="m in snapshot?.macros" :key="m.id" :value="m.id">
                {{ t('Макрос') }}{{ m.id + 1 }} · {{ m.steps.length }}{{ t('действий') }}
              </option>
            </select>
            <label v-if="kind === 'macro'"
              >{{ t('Повторов') }}<input v-model.number="repeats" type="number" min="1" max="255"
            /></label>
            <button
              class="primary full"
              :disabled="
                kind === 'macro' &&
                !snapshot?.macros.some((m) => m.id === macroId && m.steps.length)
              "
              @click="bind"
            >
              {{ t('Назначить в черновике') }}
            </button>
          </div>
        </details>
      </template>
      <template v-else-if="section === 'lighting'">
        <h3>{{ t('Цвет выбранных клавиш') }}</h3>
        <div class="color-input">
          <input v-model="color" type="color" :aria-label="t('Цвет клавиш')" /><input
            v-model="color"
            maxlength="7"
            :aria-label="t('HEX цвета')"
          />
        </div>
        <button
          class="primary full"
          :disabled="!/^#[0-9a-f]{6}$/i.test(color)"
          @click="emit('stage', { kind: 'color', slots: selected, color: parseColor(color) })"
        >
          {{ t('Окрасить в черновике') }}
        </button>
        <p class="hint">
          {{
            t(
              'Индивидуальные цвета использует эффект «Свои цвета». Текущий свет устройства может отличаться.',
            )
          }}
        </p>
      </template>
      <template v-else-if="section === 'advanced'">
        <label
          >{{ t('Поведение')
          }}<select v-model="advanced">
            <option value="mt">{{ t('MT · удержание и касание') }}</option>
            <option value="tgl">{{ t('TGL · переключатель') }}</option>
            <option value="rs">{{ t('RS · глубже нажатая клавиша') }}</option>
            <option value="socd">{{ t('SOCD · приоритет пары') }}</option>
          </select></label
        >
        <p class="hint">
          {{
            t(
              String(
                advanced === 'mt'
                  ? 'Разные действия для удержания и короткого нажатия.'
                  : advanced === 'tgl'
                    ? 'Короткое нажатие переключает удержание действия.'
                    : 'Выберите ровно две физические клавиши.',
              ),
            )
          }}
        </p>
        <template v-if="advanced === 'mt' || advanced === 'tgl'"
          ><label v-if="advanced === 'mt'"
            >{{ t('Удержание')
            }}<select v-model.number="holdCode">
              <option v-for="k in firmwareActionChoices" :key="k.code" :value="k.code">
                {{ k.label }}
              </option>
            </select></label
          ><label
            >{{ t(String(advanced === 'mt' ? 'Касание' : 'Действие'))
            }}<select v-model.number="tapCode">
              <option v-for="k in firmwareActionChoices" :key="k.code" :value="k.code">
                {{ k.label }}
              </option>
            </select></label
          ><label v-if="advanced === 'mt'"
            >{{ t('Граница удержания ·') }}{{ delay }}{{ t('мс')
            }}<input v-model.number="delay" type="range" min="10" max="1000" step="10" /></label
        ></template>
        <label v-if="advanced === 'socd'"
          >{{ t('Режим приоритета')
          }}<select v-model.number="socd">
            <option :value="3">{{ t('Последняя нажатая') }}</option>
            <option :value="1">{{ t('Первая клавиша пары') }}</option>
            <option :value="2">{{ t('Вторая клавиша пары') }}</option>
            <option :value="4">{{ t('Нейтральный') }}</option>
          </select></label
        >
        <button
          class="primary full"
          :disabled="
            ['rs', 'socd'].includes(advanced) ? selected.length !== 2 : selected.length !== 1
          "
          @click="advancedEdit"
        >
          {{ t('Добавить поведение') }}
        </button>
      </template>
      <template v-else
        ><p class="hint">
          {{ t('Выбранные клавиши сохраняются при переходе между редакторами.') }}
        </p>
        <div class="selected-key-details">
          <span>{{ t('Порог') }}</span
          ><b>{{ ((key?.actuation.triggerUm ?? 0) / 1000).toFixed(2) }}{{ t('мм') }}</b
          ><span>{{ t('Заданный цвет') }}</span
          ><b>{{ key ? hexColor(key.color) : '—' }}</b>
        </div></template
      >
    </fieldset>
  </aside>
</template>
