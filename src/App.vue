<script setup lang="ts">
import { computed, nextTick, onMounted, onBeforeUnmount, ref, watch } from 'vue';
import KeyboardPreview from './shared/keyboard-view/KeyboardPreview.vue';
import Inspector from './features/editor/Inspector.vue';
import MacroEditor from './features/editor/MacroEditor.vue';
import DksEditor from './features/editor/DksEditor.vue';
import { keyboardKeys } from './shared/keyboard-view/layout';
import { useWorkspace } from './features/editor/workspace';
import { importVisionProfile } from './features/editor/io-vision-profile';
import {
  clone,
  bindingName,
  effects,
  hexColor,
  mixColor,
  parseColor,
  profileEdits,
  readLocalProfile,
  type LocalProfile,
} from './features/editor/model';
import type {
  Edit,
  KeyboardSnapshot,
  LightingSettings,
  PerformanceSettings,
} from './shared/contracts/generated';

const workspace = useWorkspace();
const {
  native,
  snapshot,
  draft,
  edits,
  redo,
  busy,
  error,
  notice,
  connected,
  preview,
  live,
  displayLive,
  profiles,
  recoveryPreview,
} = workspace;
const section = ref('overview'),
  view = ref<'layout' | 'travel' | 'colors'>('layout'),
  selected = ref<number[]>([]),
  functionLayer = ref(false),
  multi = ref(false),
  actualColors = ref(false),
  search = ref('');
const profileName = ref('Мой профиль'),
  confirmDiscard = ref(false),
  confirmRefresh = ref(false);
const importNotes = ref<string[]>([]);
const gradientStart = ref('#a78bfa'),
  gradientEnd = ref('#5eead4'),
  gradientDirection = ref('horizontal');
const light = ref<LightingSettings>({
  mode: 11,
  color: { r: 255, g: 255, b: 255 },
  secondaryColor: { r: 0, g: 0, b: 0 },
  colorMode: 1,
  brightness: 5,
  speed: 3,
  direction: 0,
});
const performance = ref<PerformanceSettings>({
  reportRate: 6,
  topDeadZoneUm: 0,
  bottomDeadZoneUm: 0,
  keyDelay: 0,
});
const items = [
  { id: 'overview', label: 'Рабочий стол', icon: '◫' },
  { id: 'keys', label: 'Назначения', icon: '⌘' },
  { id: 'actuation', label: 'Ход и Rapid Trigger', icon: '↕' },
  { id: 'lighting', label: 'Свет', icon: '◉' },
  { id: 'advanced', label: 'Расширенные клавиши', icon: '◇' },
  { id: 'macros', label: 'Макросы', icon: '≋' },
  { id: 'profiles', label: 'Профили', icon: '▤' },
];
const title = computed(() => items.find((i) => i.id === section.value)?.label ?? 'Параметры');
const keyLabel = (slot: number) =>
  keyboardKeys.find((k) => k.slot === slot)?.label ?? `Слот ${slot}`;
const previewDetails = computed(() => {
  if (!snapshot.value || !draft.value || recoveryPreview.value) return [];
  const before = snapshot.value,
    after = draft.value,
    lines: string[] = [];
  const changed = (a: unknown, b: unknown) => JSON.stringify(a) !== JSON.stringify(b);
  for (const { slot } of keyboardKeys) {
    const a = before.keys[slot]!,
      b = after.keys[slot]!,
      label = keyLabel(slot);
    for (const layer of ['base', 'function'] as const)
      if (changed(a[layer], b[layer]))
        lines.push(
          `${layer === 'function' ? 'Fn + ' : ''}${label}: ${bindingName(a[layer], label)} → ${bindingName(b[layer], label)}`,
        );
    if (changed(a.actuation, b.actuation))
      lines.push(
        `${label}: ход ${(a.actuation.triggerUm / 1000).toFixed(2)} → ${(b.actuation.triggerUm / 1000).toFixed(2)} мм; RT ${b.actuation.rapidTrigger ? `${(b.actuation.pressUm / 1000).toFixed(2)} / ${(b.actuation.releaseUm / 1000).toFixed(2)} мм` : 'выключен'}`,
      );
    if (changed(a.color, b.color))
      lines.push(`${label}: цвет ${hexColor(a.color)} → ${hexColor(b.color)}`);
  }
  if (changed(before.lighting, after.lighting))
    lines.push(
      `Свет: ${effects[before.lighting.mode] ?? before.lighting.mode} → ${effects[after.lighting.mode] ?? after.lighting.mode}; яркость ${before.lighting.brightness} → ${after.lighting.brightness}; скорость ${before.lighting.speed} → ${after.lighting.speed}; цвет ${hexColor(after.lighting.color)}`,
    );
  if (changed(before.performance, after.performance))
    lines.push(
      `Частота ${{ 3: 1000, 5: 4000, 6: 8000 }[after.performance.reportRate] ?? '?'} Гц; мёртвые зоны ${(after.performance.topDeadZoneUm / 1000).toFixed(2)} / ${(after.performance.bottomDeadZoneUm / 1000).toFixed(2)} мм`,
    );
  if (changed(before.macros, after.macros))
    lines.push(
      `Каталог макросов: ${before.macros.length} → ${after.macros.length}; всего ${after.macros.reduce((n, m) => n + m.steps.length, 0)} действий`,
    );
  const dks = after.dks.filter((d) => changed(before.dks[d.index], d));
  if (dks.length) lines.push(`Изменены записи DKS: ${dks.map((d) => d.index + 1).join(', ')}`);
  return lines;
});
const focused = computed(() => displayLive.value.travel.find((k) => k.slot === selected.value[0]));
const freshTravel = computed(() =>
  live.value.active ? displayLive.value.travel.filter((k) => k.ageMs < 600) : [],
);
const moving = computed(() => freshTravel.value.filter((k) => k.travelUm >= 300));
const colorHex = computed({
  get: () => hexColor(light.value.color),
  set: (value: string) => {
    if (/^#[0-9a-f]{6}$/i.test(value)) light.value.color = parseColor(value);
  },
});
const customColorCount = computed(
  () => draft.value?.keys.filter((k) => k.color.r || k.color.g || k.color.b).length ?? 0,
);
watch(
  draft,
  (value) => {
    if (value) {
      light.value = clone(value.lighting);
      performance.value = clone(value.performance);
    }
  },
  { immediate: true },
);
watch(section, (value) => {
  view.value = value === 'lighting' ? 'colors' : value === 'actuation' ? 'travel' : 'layout';
  window.scrollTo({ top: 0, behavior: 'auto' });
});
function select(slot: number, toggle: boolean, paint = false) {
  if (paint) {
    if (!selected.value.includes(slot)) selected.value.push(slot);
    return;
  }
  if (toggle)
    selected.value = selected.value.includes(slot)
      ? selected.value.filter((s) => s !== slot)
      : [...selected.value, slot];
  else selected.value = [slot];
}
function group(name: string) {
  selected.value =
    name === 'all'
      ? keyboardKeys.map((k) => k.slot)
      : name === 'wasd'
        ? [34, 49, 50, 51]
        : name === 'arrows'
          ? [88, 89, 90, 91]
          : [];
}
function findKey() {
  const found =
    keyboardKeys.find((k) => k.label.toLowerCase() === search.value.toLowerCase().trim()) ??
    keyboardKeys.find((k) => k.label.toLowerCase().includes(search.value.toLowerCase().trim()));
  if (found) selected.value = [found.slot];
}
function gradient() {
  if (!draft.value) return;
  const keys = keyboardKeys.filter(
    (k) => !selected.value.length || selected.value.includes(k.slot),
  );
  const coord = (k: (typeof keyboardKeys)[number]) =>
    gradientDirection.value === 'horizontal' ? k.x + k.width / 2 : k.y + k.height / 2;
  const positions = keys.map(coord),
    min = Math.min(...positions),
    max = Math.max(...positions);
  const a = parseColor(gradientStart.value),
    b = parseColor(gradientEnd.value);
  const changes: Edit[] = keys.map((k) => ({
    kind: 'color',
    slots: [k.slot],
    color: mixColor(a, b, max === min ? 0 : (coord(k) - min) / (max - min)),
  }));
  changes.push({ kind: 'lighting', value: { ...clone(light.value), mode: 20 } });
  workspace.stage(...changes);
  actualColors.value = false;
}
function loadProfile(profile: LocalProfile) {
  try {
    if (snapshot.value) {
      workspace.stage(...profileEdits(draft.value!, profile.snapshot));
      notice.value = 'Профиль добавлен в черновик. Устройство ещё не изменено.';
    } else {
      snapshot.value = clone(profile.snapshot);
      notice.value = 'Локальный профиль открыт без подключения';
    }
    profileName.value = profile.name;
  } catch (e) {
    error.value = String(e);
  }
}
async function importProfile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  try {
    if (file.size > 2_000_000) throw new Error('Файл слишком большой');
    const text = await file.text();
    const root: unknown = JSON.parse(text);
    importNotes.value = [];
    if (root && typeof root === 'object' && 'deviceId' in root) {
      if (!draft.value)
        throw new Error(
          'Для импорта IO Vision сначала прочитайте клавиатуру: неоднозначные поля нужно сохранить из исходного снимка.',
        );
      const imported = importVisionProfile(text, draft.value);
      workspace.stage(...imported.edits);
      profileName.value = imported.name;
      importNotes.value = imported.warnings;
      notice.value = `Импорт IO Vision: ${imported.edits.length} изменений в черновике`;
    } else loadProfile(readLocalProfile(text));
  } catch (e) {
    error.value = String(e);
  }
  input.value = '';
}
function openExample() {
  const template: KeyboardSnapshot = {
    revision: 'local-template',
    identity: {
      name: 'IO Type 84 Magnetic White',
      vendorId: 0xc45,
      productId: 0x80d6,
      firmware: '—',
      frameVersion: 0,
      rtPrecision: 0,
    },
    keys: Array.from({ length: 128 }, (_, slot) => ({
      slot,
      base: { page: 0, parameters: [0, 0, 0] },
      function: { page: 0, parameters: [0, 0, 0] },
      actuation: {
        triggerUm: 1200,
        pressUm: 0,
        releaseUm: 0,
        rapidTrigger: false,
        wholeTravel: false,
        rampage: false,
        axisType: 0,
      },
      ledId: slot,
      color: { r: 0, g: 0, b: 0 },
    })),
    lighting: clone(light.value),
    performance: clone(performance.value),
    macros: [],
    dks: Array.from({ length: 64 }, (_, index) => ({
      index,
      thresholds: [0, 0, 0, 0],
      actions: [0, 0, 0, 0],
      states: [0, 0, 0, 0],
    })),
    macroBytesUsed: 400,
    macroWriteLimit: 512,
  };
  snapshot.value = template;
  notice.value = 'Пример профиля · значения не прочитаны с устройства';
  selected.value = [34, 49, 50, 51];
}
function shortcuts(event: KeyboardEvent) {
  if (busy.value || modalOpen.value) return;
  if (
    event.target instanceof HTMLInputElement ||
    event.target instanceof HTMLTextAreaElement ||
    event.target instanceof HTMLSelectElement
  )
    return;
  if ((event.ctrlKey || event.metaKey) && event.code === 'KeyZ') {
    event.preventDefault();
    event.shiftKey ? workspace.redoEdit() : workspace.undo();
  }
  if ((event.ctrlKey || event.metaKey) && event.code === 'KeyS') {
    event.preventDefault();
    workspace.saveProfile(profileName.value);
  }
  if (event.code === 'Escape') {
    selected.value = [];
    preview.value = null;
  }
}
let timer: ReturnType<typeof setInterval> | undefined;
onMounted(() => {
  timer = setInterval(workspace.poll, 60);
  window.addEventListener('keydown', shortcuts);
});
onBeforeUnmount(() => {
  if (timer) clearInterval(timer);
  window.removeEventListener('keydown', shortcuts);
  if (connected.value) void workspace.disconnect();
});
const blockNames: Record<string, string> = {
  game: 'Общие параметры',
  base: 'Базовый слой',
  function: 'Fn-слой',
  lighting: 'Эффект подсветки',
  colors: 'Индивидуальные цвета',
  actuation: 'Ход и RT',
  dks: 'Динамические нажатия',
  macros: 'Макросы',
};
const dialog = ref<HTMLElement | null>(null);
const modalOpen = computed(() => !!preview.value || confirmDiscard.value || confirmRefresh.value);
let returnFocus: HTMLElement | null = null;
watch(modalOpen, async (open) => {
  if (open) {
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    await nextTick();
    dialog.value?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
  } else returnFocus?.focus();
});
function modalKeys(event: KeyboardEvent) {
  if (event.key === 'Escape' && !busy.value) {
    event.preventDefault();
    preview.value = null;
    confirmDiscard.value = false;
    confirmRefresh.value = false;
  }
  if (event.key !== 'Tab') return;
  const buttons = dialog.value?.querySelectorAll<HTMLElement>(
    'button:not(:disabled), input:not(:disabled), select:not(:disabled), [tabindex="0"]',
  );
  const first = buttons?.[0],
    last = buttons?.[buttons.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last?.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first?.focus();
  }
}
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <a class="brand" href="#" @click.prevent="section = 'overview'"
        ><span class="brand-mark">io</span>
        <div>TYPE 84<small>Личное пространство</small></div></a
      >
      <div class="device-card">
        <div class="device-line">
          <span class="device-mini">⌨</span
          ><span class="status-dot" :class="{ online: connected }"></span>
        </div>
        <b>Magnetic White</b
        ><small>{{
          connected
            ? 'USB · прошивка ' + snapshot?.identity.firmware
            : native
              ? 'Готова к подключению'
              : 'Предпросмотр в браузере'
        }}</small
        ><button
          v-if="native"
          class="connect-button"
          :disabled="!!busy"
          @click="connected ? workspace.disconnect() : workspace.connect()"
        >
          {{ connected ? 'Отключить' : busy || 'Подключить клавиатуру' }} <span>↗</span>
        </button>
      </div>
      <div class="nav-label">ВАША КЛАВИАТУРА</div>
      <nav aria-label="Разделы">
        <button
          v-for="item in items"
          :key="item.id"
          class="nav-item"
          :class="{ active: section === item.id }"
          :aria-current="section === item.id ? 'page' : undefined"
          @click="section = item.id"
        >
          <span>{{ item.icon }}</span
          >{{ item.label }}
        </button>
      </nav>
      <div class="sidebar-bottom">
        <div class="local-badge">
          <span>◇</span>
          <div>Всё на вашем компьютере<small>Профили и история сеанса</small></div>
        </div>
        <button
          class="nav-item"
          :class="{ active: section === 'settings' }"
          @click="section = 'settings'"
        >
          <span>⚙</span>Параметры
        </button>
        <div class="sidebar-version">Открытый проект <span>0.1.0</span></div>
      </div>
    </aside>
    <div class="workspace">
      <header class="topbar">
        <div class="breadcrumb">
          IO Type 84 <span>/</span><strong>{{ title }}</strong>
        </div>
        <div class="top-actions">
          <span class="connection-pill" :class="{ online: connected }"
            ><i></i
            >{{ connected ? 'Подключена' : snapshot ? 'Локальный снимок' : 'Без устройства' }}</span
          ><button
            class="icon-button"
            :disabled="!connected || !!busy"
            title="Перечитать состояние"
            @click="edits.length ? (confirmRefresh = true) : workspace.refresh()"
          >
            ↻
          </button>
        </div>
      </header>
      <main>
        <div class="page-heading">
          <div>
            <p class="eyebrow">НАСТРОЙТЕ ПОД СВОЙ РИТМ</p>
            <h1>{{ section === 'overview' ? 'Каждая клавиша — ваша.' : title }}</h1>
            <p class="page-description">
              {{
                section === 'overview'
                  ? 'Настройки, живой ход и свет — в одном пространстве.'
                  : section === 'lighting'
                    ? 'От одного цвета до градиента на всей клавиатуре.'
                    : section === 'actuation'
                      ? 'Найдите точку, в которой движение становится действием.'
                      : section === 'keys'
                        ? 'Одно действие или группа клавиш. Базовый слой и Fn.'
                        : section === 'advanced'
                          ? 'Разные действия по ходу, времени и сочетанию нажатий.'
                          : section === 'macros'
                            ? 'Соберите последовательность и назначьте её на клавишу.'
                            : section === 'profiles'
                              ? 'Сохраните привычные настройки и возвращайтесь к ним.'
                              : 'Подключение, производительность и восстановление.'
              }}
            </p>
          </div>
          <button
            v-if="connected"
            class="monitor-button"
            :class="{ active: live.active }"
            :disabled="!!busy"
            @click="workspace.monitor()"
          >
            <span>{{ live.active ? '■' : '◉' }}</span
            >{{ live.active ? 'Остановить наблюдение' : 'Наблюдать нажатия' }}
          </button>
        </div>
        <div v-if="error" class="message error-message" role="alert">
          <span>!</span>
          <p>{{ error }}</p>
          <button title="Скрыть ошибку" @click="error = ''">×</button>
        </div>
        <div v-else-if="notice" class="message notice-message" role="status">
          <span>✓</span>
          <p>{{ notice }}</p>
          <button title="Скрыть уведомление" @click="notice = ''">×</button>
        </div>
        <section
          v-if="importNotes.length"
          class="surface import-report"
          aria-label="Результат импорта"
        >
          <h3>Результат импорта IO Vision</h3>
          <ul>
            <li v-for="note in importNotes" :key="note">{{ note }}</li>
          </ul>
          <button class="text-button" @click="importNotes = []">Скрыть отчёт</button>
        </section>
        <div v-if="!snapshot" class="welcome-card">
          <div>
            <p class="eyebrow">НАЧНЁМ С ПОДКЛЮЧЕНИЯ</p>
            <h2>Познакомимся с вашей клавиатурой</h2>
            <p>
              {{
                native
                  ? 'Подключите IO White по USB. Приложение прочитает настройки и откроет редакторы.'
                  : 'В браузере доступны локальные профили и пример редактора. Для USB запустите настольное приложение.'
              }}
            </p>
            <div class="welcome-actions">
              <button v-if="native" class="primary" :disabled="!!busy" @click="workspace.connect()">
                {{ busy || 'Подключить по USB' }}</button
              ><button class="secondary" @click="openExample">Открыть пример профиля</button
              ><label class="text-button file-button"
                >Импорт JSON<input
                  type="file"
                  accept=".json,application/json"
                  @change="importProfile"
              /></label>
            </div>
          </div>
          <div class="welcome-graphic" aria-hidden="true">
            <i>W</i>
            <div><i>A</i><i>S</i><i>D</i></div>
            <span>Всё начинается с одного нажатия</span>
          </div>
        </div>
        <div v-if="!['profiles', 'settings'].includes(section)" class="editor-grid">
          <div class="editor-main">
            <section class="surface keyboard-surface">
              <div class="keyboard-heading">
                <div>
                  <h2>Ваша раскладка</h2>
                  <span>{{ functionLayer ? 'Fn-слой' : 'Базовый слой' }} · 84 клавиши</span>
                </div>
                <div class="segmented" aria-label="Слой">
                  <button :class="{ active: !functionLayer }" @click="functionLayer = false">
                    Базовый</button
                  ><button :class="{ active: functionLayer }" @click="functionLayer = true">
                    Fn
                  </button>
                </div>
              </div>
              <div class="keyboard-toolbar">
                <div class="segmented">
                  <button :class="{ active: view === 'layout' }" @click="view = 'layout'">
                    Назначения</button
                  ><button :class="{ active: view === 'travel' }" @click="view = 'travel'">
                    Ход</button
                  ><button :class="{ active: view === 'colors' }" @click="view = 'colors'">
                    Цвета
                  </button>
                </div>
                <select v-if="view === 'colors'" v-model="actualColors" aria-label="Источник цвета">
                  <option :value="false">Заданный цвет</option>
                  <option :value="true">Сообщённый устройством</option></select
                ><span v-else class="quiet-label">{{
                  live.active ? '● Измерение активно' : 'IO Type 84 Magnetic'
                }}</span>
              </div>
              <KeyboardPreview
                :snapshot="draft"
                :selected="selected"
                :live="displayLive"
                :view="view"
                :function-layer="functionLayer"
                :multi="multi"
                :actual-colors="actualColors"
                @select="select"
              />
              <div class="selection-toolbar">
                <span>Выбрать</span><button @click="group('all')">Все</button
                ><button @click="group('wasd')">WASD</button
                ><button @click="group('arrows')">Стрелки</button
                ><button :disabled="!selected.length" @click="group('none')">Снять</button
                ><label class="check-line"><input v-model="multi" type="checkbox" />Группа</label>
                <form class="key-search" @submit.prevent="findKey">
                  <input
                    v-model="search"
                    placeholder="Найти клавишу"
                    aria-label="Поиск физической клавиши"
                  /><button title="Найти">⌕</button>
                </form>
              </div>
            </section>
            <section
              v-if="section === 'overview' || section === 'actuation'"
              class="live-dashboard"
            >
              <div class="surface live-meter">
                <div class="panel-heading">
                  <h3>Живой ход</h3>
                  <span class="live-tag" :class="{ active: live.active }">{{
                    live.active ? 'LIVE' : 'ПАУЗА'
                  }}</span>
                </div>
                <div class="depth-number">
                  {{
                    live.active && focused && focused.ageMs < 1500
                      ? (focused.travelUm / 1000).toFixed(2)
                      : '—'
                  }}<small>мм</small
                  ><span>{{
                    selected.length === 1 ? keyLabel(selected[0]!) : 'Выберите одну клавишу'
                  }}</span>
                </div>
                <div class="depth-line">
                  <i
                    :style="{
                      width: `${live.active && focused && focused.ageMs < 1500 ? Math.min(focused.travelUm / 3200, 1) * 100 : 0}%`,
                    }"
                  ></i>
                </div>
                <p>
                  {{
                    live.active
                      ? `${moving.length} в движении · ${freshTravel.length} свежих измерений`
                      : 'Запустите наблюдение, чтобы увидеть глубину нажатия.'
                  }}
                </p>
              </div>
              <div class="surface session-history">
                <div class="panel-heading">
                  <h3>
                    Последние нажатия <span>{{ live.history.length }}/20</span>
                  </h3>
                  <button
                    class="text-button"
                    :disabled="!live.history.length"
                    @click="workspace.clearHistory()"
                  >
                    Очистить
                  </button>
                </div>
                <div v-if="live.history.length" class="history-chips">
                  <button
                    v-for="press in live.history"
                    :key="press.sequence"
                    @click="selected = [press.slot]"
                  >
                    <b>{{ keyLabel(press.slot) }}</b
                    ><small>{{ (press.peakUm / 1000).toFixed(2) }} мм</small>
                  </button>
                </div>
                <div v-else class="history-empty">
                  <span>↓</span>
                  <p>
                    Здесь появятся ваши нажатия<small
                      >Храним только последние 20 событий в памяти.</small
                    >
                  </p>
                </div>
                <p class="hint">
                  История физического движения от 0,10 мм; она не заменяет события ввода и логику
                  RT.
                </p>
              </div>
            </section>
            <p
              v-if="live.message && (section === 'overview' || section === 'lighting')"
              class="hint live-note"
            >
              {{ live.message }}
            </p>
            <template v-if="section === 'lighting' && draft"
              ><section class="surface editor-panel">
                <div class="panel-heading">
                  <div>
                    <p class="eyebrow">ЭФФЕКТ НА КЛАВИАТУРЕ</p>
                    <h2>Свет с характером</h2>
                  </div>
                  <span class="tag">{{ effects[draft.lighting.mode] ?? 'Неизвестный режим' }}</span>
                </div>
                <div class="lighting-controls">
                  <div class="effect-grid">
                    <button
                      v-for="(name, index) in effects"
                      :key="index"
                      :class="{ chosen: light.mode === index }"
                      @click="light.mode = index"
                    >
                      <span class="effect-symbol">{{
                        [
                          '○',
                          '●',
                          '↓',
                          '↑',
                          '✧',
                          '⋮',
                          '◉',
                          '◌',
                          '◐',
                          '◎',
                          '↔',
                          '≈',
                          '↻',
                          '✺',
                          'ϟ',
                          '◯',
                          '≋',
                          '◍',
                          '╱',
                          '⇄',
                          '▦',
                        ][index]
                      }}</span
                      >{{ name }}
                    </button>
                  </div>
                  <div class="light-sliders">
                    <label
                      >Яркость <span>{{ light.brightness }} / 5</span
                      ><input
                        v-model.number="light.brightness"
                        type="range"
                        min="0"
                        max="5" /></label
                    ><label
                      >Скорость <span>{{ light.speed }} / 5</span
                      ><input v-model.number="light.speed" type="range" min="0" max="5" /></label
                    ><label
                      >Режим цвета<select v-model.number="light.colorMode">
                        <option :value="0">Один цвет</option>
                        <option :value="1">RGB</option>
                      </select></label
                    ><label
                      >Направление<select v-model.number="light.direction">
                        <option :value="0">Вперёд</option>
                        <option :value="1">Назад</option>
                      </select></label
                    >
                    <div class="color-input">
                      <input v-model="colorHex" type="color" aria-label="Цвет эффекта" /><input
                        v-model="colorHex"
                        maxlength="7"
                        aria-label="HEX эффекта"
                      />
                    </div>
                    <button
                      class="primary full"
                      :disabled="!!busy"
                      @click="workspace.stage({ kind: 'lighting', value: clone(light) })"
                    >
                      Добавить эффект в черновик
                    </button>
                  </div>
                </div>
              </section>
              <section class="surface editor-panel">
                <div class="panel-heading">
                  <div>
                    <p class="eyebrow">ВАША ПАЛИТРА</p>
                    <h2>Градиент одним движением</h2>
                  </div>
                  <span class="tag">{{ selected.length || 84 }} клавиш</span>
                </div>
                <div
                  class="gradient-preview"
                  :style="{
                    background: `linear-gradient(${gradientDirection === 'horizontal' ? '90' : '180'}deg,${gradientStart},${gradientEnd})`,
                  }"
                ></div>
                <div class="gradient-controls">
                  <label>Начало<input v-model="gradientStart" type="color" /></label><span>→</span
                  ><label>Конец<input v-model="gradientEnd" type="color" /></label
                  ><label
                    >Направление<select v-model="gradientDirection">
                      <option value="horizontal">Слева направо</option>
                      <option value="vertical">Сверху вниз</option>
                    </select></label
                  ><button class="primary" @click="gradient">Создать градиент</button>
                </div>
                <p class="hint">
                  {{ customColorCount }} слотов с заданным RGB. Градиент включит «Свои цвета».
                  Изменения пока останутся в черновике.
                </p>
              </section>
              <section class="surface panel-research">
                <span>▥</span>
                <div>
                  <h3>LED-панель</h3>
                  <p>
                    В профиле IO: Fn+G — глубина RT, Fn+K — наложение, Fn+L — эффект полосы. Эти
                    действия доступны в «Назначения → Действие клавиатуры / панели». Прямое
                    управление цветами и геометрия панели ещё исследуются.
                  </p>
                </div>
              </section></template
            >
            <DksEditor
              v-if="section === 'advanced' && draft"
              :snapshot="draft"
              :selected="selected"
              :function-layer="functionLayer"
              :disabled="!!busy"
              @stage="workspace.stage"
            />
            <MacroEditor
              v-if="section === 'macros' && draft"
              :snapshot="draft"
              :disabled="!!busy"
              @stage="workspace.stage"
            />
          </div>
          <Inspector
            :snapshot="draft"
            :selected="selected"
            :section="section"
            :function-layer="functionLayer"
            :disabled="!!busy"
            @stage="workspace.stage"
          />
        </div>
        <section v-if="section === 'profiles'" class="surface editor-panel">
          <div class="panel-heading">
            <div>
              <p class="eyebrow">ЛОКАЛЬНАЯ БИБЛИОТЕКА</p>
              <h2>Ваши привычные настройки</h2>
            </div>
            <span class="tag">{{ profiles.length }} профилей</span>
          </div>
          <div class="profile-create">
            <input
              v-model="profileName"
              maxlength="100"
              placeholder="Название профиля"
              aria-label="Название профиля"
            /><button
              class="primary"
              :disabled="!draft"
              @click="workspace.saveProfile(profileName)"
            >
              Сохранить текущий</button
            ><button
              class="secondary"
              :disabled="!draft"
              @click="workspace.exportProfile(profileName)"
            >
              Экспорт JSON</button
            ><label class="secondary file-button"
              >Импорт<input type="file" accept=".json,application/json" @change="importProfile"
            /></label>
          </div>
          <p class="hint">
            Ctrl+S сохраняет локальный профиль. Применение к клавиатуре выполняется отдельно. Формат
            Поддерживаются собственный JSON и проверенные поля экспорта IO Vision. Неоднозначные
            поля сайта сохраняют значения исходного снимка.
          </p>
          <div v-if="profiles.length" class="profile-grid">
            <article v-for="profile in profiles" :key="profile.name" class="profile-card">
              <span>▤</span>
              <h3>{{ profile.name }}</h3>
              <p>{{ new Date(profile.savedAt).toLocaleString('ru-RU') }}</p>
              <div>
                <button class="secondary" @click="loadProfile(profile)">Открыть в черновике</button
                ><button class="text-button" @click="workspace.deleteProfile(profile.name)">
                  Удалить
                </button>
              </div>
            </article>
          </div>
          <p v-else class="empty-inline">
            Сохраните первую настройку, чтобы быстро вернуться к ней.
          </p>
        </section>
        <section v-if="section === 'settings'" class="settings-grid">
          <div class="surface editor-panel">
            <p class="eyebrow">ПРОИЗВОДИТЕЛЬНОСТЬ</p>
            <h2>Общие параметры</h2>
            <fieldset :disabled="!draft || !!busy">
              <label
                >Частота опроса<select v-model.number="performance.reportRate">
                  <option :value="3">1000 Гц</option>
                  <option :value="5">4000 Гц</option>
                  <option :value="6">8000 Гц</option>
                </select></label
              ><label
                >Верхняя мёртвая зона, мм<input
                  :value="performance.topDeadZoneUm / 1000"
                  type="number"
                  min="0"
                  max="0.5"
                  step="0.01"
                  @change="
                    performance.topDeadZoneUm = Math.round(
                      Number(($event.target as HTMLInputElement).value) * 1000,
                    )
                  " /></label
              ><label
                >Нижняя мёртвая зона, мм<input
                  :value="performance.bottomDeadZoneUm / 1000"
                  type="number"
                  min="0"
                  max="0.5"
                  step="0.01"
                  @change="
                    performance.bottomDeadZoneUm = Math.round(
                      Number(($event.target as HTMLInputElement).value) * 1000,
                    )
                  " /></label
              ><button
                class="primary"
                @click="workspace.stage({ kind: 'performance', value: clone(performance) })"
              >
                Добавить в черновик
              </button>
            </fieldset>
          </div>
          <div class="surface editor-panel">
            <p class="eyebrow">КОНТРОЛЬ И ВОССТАНОВЛЕНИЕ</p>
            <h2>Состояние устройства</h2>
            <dl class="device-facts">
              <dt>Модель</dt>
              <dd>IO White</dd>
              <dt>Прошивка</dt>
              <dd>{{ snapshot?.identity.firmware ?? '—' }}</dd>
              <dt>Протокол</dt>
              <dd>io_vision_v0</dd>
              <dt>Текущие RGB</dt>
              <dd>{{ live.colors.length ? 'Получены от устройства' : 'Не подтверждены' }}</dd>
              <dt>Пакеты хода</dt>
              <dd>{{ live.packets.toLocaleString('ru-RU') }}</dd>
            </dl>
            <p class="hint">
              Перед записью сохраняется резервный снимок в каталоге приложения. Восстановление
              сначала покажет список блоков.
            </p>
            <button
              class="secondary"
              :disabled="!connected || !!busy"
              @click="workspace.recovery()"
            >
              Подготовить восстановление
            </button>
          </div>
        </section>
        <footer class="workspace-footer">
          <span>Создано для вашей IO Type 84</span
          ><span>Настройки устройства · локальные профили · Windows USB</span>
        </footer>
      </main>
      <div v-if="snapshot" class="draft-bar">
        <div>
          <span class="draft-dot" :class="{ dirty: edits.length }"></span
          ><b>{{ edits.length ? 'Есть изменения в черновике' : 'Черновик без изменений' }}</b
          ><small>{{
            edits.length
              ? 'Клавиатура изменится после применения'
              : 'Выберите клавишу и настройте её под себя'
          }}</small>
        </div>
        <div class="draft-actions">
          <button
            class="icon-button"
            :disabled="!edits.length || !!busy"
            title="Отменить действие · Ctrl+Z"
            @click="workspace.undo()"
          >
            ↶</button
          ><button
            class="icon-button"
            :disabled="!redo.length || !!busy"
            title="Вернуть действие · Ctrl+Shift+Z"
            @click="workspace.redoEdit()"
          >
            ↷</button
          ><button
            class="text-button"
            :disabled="!edits.length || !!busy"
            @click="confirmDiscard = true"
          >
            Сбросить черновик</button
          ><button
            class="primary"
            :disabled="!connected || !edits.length || !!busy"
            @click="workspace.prepare()"
          >
            {{ busy || 'Проверить и применить' }}
          </button>
        </div>
      </div>
    </div>
    <div
      v-if="preview || confirmDiscard || confirmRefresh"
      class="modal-backdrop"
      @click.self="!busy && ((preview = null), (confirmDiscard = false), (confirmRefresh = false))"
    >
      <section
        ref="dialog"
        class="dialog surface"
        role="dialog"
        aria-modal="true"
        @keydown="modalKeys"
        :aria-label="preview ? 'Применение изменений' : 'Подтверждение действия'"
      >
        <template v-if="preview"
          ><p class="eyebrow">ПЕРЕД ЗАПИСЬЮ</p>
          <h2>Проверьте изменения</h2>
          <p>
            Исходное состояние будет проверено ещё раз. Затем приложение сохранит резервный снимок и
            сверит результат чтением.
          </p>
          <ul class="change-list">
            <li v-for="change in preview.changes" :key="change.block">
              <b>{{ blockNames[change.block] ?? change.block }}</b
              ><span>Будет обновлён</span>
            </li>
          </ul>
          <ul
            v-if="previewDetails.length"
            class="semantic-changes"
            tabindex="0"
            aria-label="Подробности изменений"
          >
            <li v-for="(line, index) in previewDetails" :key="index">{{ line }}</li>
          </ul>
          <p v-if="recoveryPreview" class="hint">
            Восстанавливаются исходные данные этих блоков из последней резервной копии.
          </p>
          <p v-if="!preview.changes.length">Отличий от устройства нет.</p>
          <p v-if="error" class="dialog-error" role="alert">{{ error }}</p>
          <div class="dialog-actions">
            <button class="secondary" :disabled="!!busy" @click="preview = null">Вернуться</button
            ><button
              class="primary"
              :disabled="!!busy || !preview.changes.length"
              @click="workspace.apply()"
            >
              {{ busy || 'Применить к клавиатуре' }}
            </button>
          </div></template
        ><template v-else
          ><h2>{{ confirmRefresh ? 'Перечитать настройки?' : 'Отменить весь черновик?' }}</h2>
          <p>Неприменённые изменения будут удалены. Настройки клавиатуры останутся прежними.</p>
          <div class="dialog-actions">
            <button
              class="secondary"
              @click="
                confirmDiscard = false;
                confirmRefresh = false;
              "
            >
              Оставить</button
            ><button
              class="primary"
              @click="
                confirmRefresh ? workspace.refresh() : workspace.discard();
                confirmDiscard = false;
                confirmRefresh = false;
              "
            >
              {{ confirmRefresh ? 'Перечитать' : 'Отменить черновик' }}
            </button>
          </div></template
        >
      </section>
    </div>
  </div>
</template>
