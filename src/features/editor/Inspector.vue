<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type { Actuation, Edit, KeyboardSnapshot } from '../../shared/contracts/generated';
import {
  bindingName,
  clone,
  deviceActions,
  hexColor,
  keyChoices,
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
watch(kind, (value) => {
  if (value === 'mouse' || value === 'wheel') code.value = 1;
  else if (value === 'media') code.value = 0xe9;
  else if (value === 'device') code.value = 11;
  else if (value === 'keyboard' || value === 'combo')
    code.value = physicalKeyCodes[props.selected[0] ?? -1] ?? 4;
  else if (value === 'macro') macroId.value = props.snapshot?.macros[0]?.id ?? 0;
});
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
      code.value = bound.page === 2 ? bound.parameters[1] : (physicalKeyCodes[k.slot] ?? 4);
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
  if (kind.value === 'disabled') parameters = [0, 0, 0];
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
    <div class="inspector-title">
      <span class="eyebrow">ВЫБРАНО</span>
      <h2>{{ selected.length ? selectionTitle : 'Выберите клавишу' }}</h2>
      <p v-if="key && selected.length === 1">
        {{ bindingName(key[functionLayer ? 'function' : 'base'], selectionTitle) }} ·
        {{ functionLayer ? 'Fn' : 'Базовый' }} слой
      </p>
      <p v-else>Настройка одной клавиши или всей группы.</p>
    </div>
    <div v-if="!selected.length" class="selection-empty">
      <span>⌁</span>
      <h3>Начните с клавиатуры</h3>
      <p>Нажмите на клавишу или выберите группу. Все её настройки будут здесь.</p>
    </div>
    <fieldset v-else :disabled="disabled || !snapshot" class="inspector-fields">
      <p v-if="selected.length > 1" class="hint">
        Показаны значения первой выбранной клавиши. Добавленное изменение применяется ко всей
        группе.
      </p>
      <template v-if="section === 'actuation' || section === 'overview'">
        <div class="field-heading">
          <h3>Точка активации</h3>
          <span class="value-chip">{{ (rt.triggerUm / 1000).toFixed(2) }} <small>мм</small></span>
        </div>
        <p v-if="rt.triggerUm < 100" class="hint">
          В снимке нулевой порог. Для нового назначения выберите значение от 0,10 мм.
        </p>
        <div class="travel-control">
          <div class="travel-track">
            <div :style="{ height: `${(rt.triggerUm / 3200) * 100}%` }"></div>
            <span :style="{ bottom: `${(rt.triggerUm / 3200) * 100}%` }"></span>
          </div>
          <div>
            <input
              v-model.number="rt.triggerUm"
              type="range"
              min="100"
              max="3200"
              step="10"
              aria-label="Точка активации в микрометрах"
            />
            <div class="range-labels"><span>0,10</span><span>3,20 мм</span></div>
            <p>Меньше ход — раньше срабатывание.</p>
            <label class="numeric-inline"
              >Точно, мм<input
                :value="(rt.triggerUm / 1000).toFixed(2)"
                type="number"
                min="0.1"
                max="3.2"
                step="0.01"
                @change="
                  rt.triggerUm = Math.round(
                    Number(($event.target as HTMLInputElement).value) * 1000,
                  )
                "
            /></label>
          </div>
        </div>
        <label class="switch-line"
          ><span><b>Rapid Trigger</b><small>Повторное срабатывание по движению</small></span
          ><input v-model="rt.rapidTrigger" type="checkbox" role="switch"
        /></label>
        <div v-if="rt.rapidTrigger" class="rt-settings">
          <label
            >Нажатие <span>{{ (rt.pressUm / 1000).toFixed(2) }} мм</span
            ><input v-model.number="rt.pressUm" type="range" min="10" max="3200" step="10"
          /></label>
          <label
            >Отпускание <span>{{ (rt.releaseUm / 1000).toFixed(2) }} мм</span
            ><input v-model.number="rt.releaseUm" type="range" min="10" max="3200" step="10"
          /></label>
          <label class="check-line"
            ><input v-model="rt.wholeTravel" type="checkbox" />На всём ходе (Whole Fast)</label
          ><label class="check-line"><input v-model="rt.rampage" type="checkbox" />Rampage</label>
        </div>
        <button
          class="primary full"
          @click="emit('stage', { kind: 'actuation', slots: selected, value: clone(rt) })"
        >
          Добавить в черновик
        </button>
        <p class="hint">
          Группа получает выбранные пороги. Тип датчика и неизвестные поля сохраняются.
        </p>
      </template>
      <template v-else-if="section === 'keys'">
        <label
          >Тип действия<select
            v-model="kind"
            @change="code = kind === 'mouse' ? 1 : kind === 'media' ? 0xe9 : 4"
          >
            <option value="keyboard">Клавиша</option>
            <option value="media">Медиа</option>
            <option value="device">Действие клавиатуры / панели</option>
            <option value="mouse">Кнопка мыши</option>
            <option value="wheel">Прокрутка мыши</option>
            <option value="combo">Сочетание клавиш</option>
            <option value="macro">Аппаратный макрос</option>
            <option value="default">Заводское назначение</option>
            <option value="disabled">Отключить</option>
          </select></label
        >
        <template v-if="kind === 'keyboard'"
          ><input
            v-model="search"
            class="search"
            type="search"
            placeholder="Найти клавишу…"
            aria-label="Поиск назначения"
          />
          <div class="character-grid">
            <button
              v-for="choice in choices"
              :key="choice.code"
              :class="{ chosen: code === choice.code }"
              :title="choice.group"
              @click="code = choice.code"
            >
              {{ choice.label }}
            </button>
          </div>
          <p>
            Назначение: <b>{{ keyName(code) }}</b>
          </p></template
        >
        <select v-if="kind === 'media'" v-model.number="code" aria-label="Медиа-действие">
          <option :value="0xe9">Громче</option>
          <option :value="0xea">Тише</option>
          <option :value="0xe2">Без звука</option>
          <option :value="0xcd">Воспроизведение / пауза</option>
          <option :value="0xb5">Следующий трек</option>
          <option :value="0xb6">Предыдущий трек</option>
          <option :value="0x192">Калькулятор</option>
        </select>
        <select v-if="kind === 'device'" v-model.number="code" aria-label="Действие прошивки">
          <option v-for="a in deviceActions" :key="a.code" :value="a.code">{{ a.label }}</option>
        </select>
        <select v-if="kind === 'mouse'" v-model.number="code" aria-label="Кнопка мыши">
          <option :value="1">Левая</option>
          <option :value="2">Правая</option>
          <option :value="4">Средняя</option>
          <option :value="8">Назад</option>
          <option :value="16">Вперёд</option>
        </select>
        <select v-if="kind === 'wheel'" v-model.number="code" aria-label="Направление прокрутки">
          <option :value="1">Вверх</option>
          <option :value="255">Вниз</option>
        </select>
        <template v-if="kind === 'combo'">
          <label
            >Модификатор<select v-model.number="modifier1">
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
            >Второй модификатор<select v-model.number="modifier2">
              <option :value="0">Нет</option>
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
            >Клавиша<select v-model.number="code">
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
        <select v-if="kind === 'macro'" v-model.number="macroId" aria-label="Аппаратный макрос">
          <option v-for="m in snapshot?.macros" :key="m.id" :value="m.id">
            Макрос {{ m.id + 1 }} · {{ m.steps.length }} действий
          </option>
        </select>
        <label v-if="kind === 'macro'"
          >Повторов<input v-model.number="repeats" type="number" min="1" max="255"
        /></label>
        <button
          class="primary full"
          :disabled="
            kind === 'macro' && !snapshot?.macros.some((m) => m.id === macroId && m.steps.length)
          "
          @click="bind"
        >
          Назначить в черновике
        </button>
      </template>
      <template v-else-if="section === 'lighting'">
        <h3>Цвет выбранных клавиш</h3>
        <div class="color-input">
          <input v-model="color" type="color" aria-label="Цвет клавиш" /><input
            v-model="color"
            maxlength="7"
            aria-label="HEX цвета"
          />
        </div>
        <button
          class="primary full"
          :disabled="!/^#[0-9a-f]{6}$/i.test(color)"
          @click="emit('stage', { kind: 'color', slots: selected, color: parseColor(color) })"
        >
          Окрасить в черновике
        </button>
        <p class="hint">
          Индивидуальные цвета использует эффект «Свои цвета». Текущий свет устройства может
          отличаться.
        </p>
      </template>
      <template v-else-if="section === 'advanced'">
        <label
          >Поведение<select v-model="advanced">
            <option value="mt">MT · удержание и касание</option>
            <option value="tgl">TGL · переключатель</option>
            <option value="rs">RS · глубже нажатая клавиша</option>
            <option value="socd">SOCD · приоритет пары</option>
          </select></label
        >
        <p class="hint">
          {{
            advanced === 'mt'
              ? 'Разные действия для удержания и короткого нажатия.'
              : advanced === 'tgl'
                ? 'Короткое нажатие переключает удержание действия.'
                : 'Выберите ровно две физические клавиши.'
          }}
        </p>
        <template v-if="advanced === 'mt' || advanced === 'tgl'"
          ><label v-if="advanced === 'mt'"
            >Удержание<select v-model.number="holdCode">
              <option v-for="k in keyChoices" :key="k.code" :value="k.code">{{ k.label }}</option>
            </select></label
          ><label
            >{{ advanced === 'mt' ? 'Касание' : 'Действие'
            }}<select v-model.number="tapCode">
              <option v-for="k in keyChoices" :key="k.code" :value="k.code">{{ k.label }}</option>
            </select></label
          ><label v-if="advanced === 'mt'"
            >Граница удержания, мс<input
              v-model.number="delay"
              type="number"
              min="10"
              max="1000"
              step="10" /></label
        ></template>
        <label v-if="advanced === 'socd'"
          >Режим приоритета<select v-model.number="socd">
            <option :value="3">Последняя нажатая</option>
            <option :value="1">Первая клавиша пары</option>
            <option :value="2">Вторая клавиша пары</option>
            <option :value="4">Нейтральный</option>
          </select></label
        >
        <button
          class="primary full"
          :disabled="
            ['rs', 'socd'].includes(advanced) ? selected.length !== 2 : selected.length !== 1
          "
          @click="advancedEdit"
        >
          Добавить поведение
        </button>
        <p class="hint">
          Формат взят из редактора IO. Физическое поведение проверяется после применения. Пара
          RS/SOCD изменяется целиком.
        </p>
      </template>
      <template v-else
        ><p class="hint">Выбранные клавиши сохраняются при переходе между редакторами.</p>
        <div class="selected-key-details">
          <span>Порог</span><b>{{ ((key?.actuation.triggerUm ?? 0) / 1000).toFixed(2) }} мм</b
          ><span>Заданный цвет</span><b>{{ key ? hexColor(key.color) : '—' }}</b>
        </div></template
      >
    </fieldset>
  </aside>
</template>
