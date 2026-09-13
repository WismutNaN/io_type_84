<script setup lang="ts">
import { computed, nextTick, onMounted, onBeforeUnmount, ref, watch } from 'vue';
import KeyboardPreview from './shared/keyboard-view/KeyboardPreview.vue';
import Inspector from './features/editor/Inspector.vue';
import MacroEditor from './features/editor/MacroEditor.vue';
import { t, locale, theme } from './shared/ui/preferences';
import Icon from './shared/ui/Icon.vue';
import TravelSlider from './shared/ui/TravelSlider.vue';
import KeyInspection from './features/editor/KeyInspection.vue';
import MonitorStrip from './features/editor/MonitorStrip.vue';
import DepthActions from './features/editor/DepthActions.vue';
import ActionCatalog from './features/editor/ActionCatalog.vue';
import { defaultAutomation } from './features/editor/computer-rules';
import {
  effectGroups,
  effectDescriptions,
  reactiveModes,
} from './features/editor/lighting-preview';
import { useLightingPreview } from './features/editor/use-lighting-preview';
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
const { automation, automationDirty, hardwareEdits, startActions } = workspace;
const catalogTab = ref('computer');
const page = ref('keyboard');
const section = ref('keys'),
  view = ref<'layout' | 'travel' | 'colors'>('layout'),
  selected = ref<number[]>([]),
  functionLayer = ref(false),
  multi = ref(false),
  actualColors = ref(false),
  search = ref('');
const profileName = ref('Мой профиль'),
  confirmDiscard = ref(false),
  confirmRefresh = ref(false);
const tool = ref('assignment');
watch(tool, (v) => {
  section.value = v === 'trigger' ? 'actuation' : v === 'behavior' ? 'advanced' : 'keys';
});
watch(page, (v) => {
  if (v === 'lighting') {
    section.value = 'lighting';
    view.value = 'colors';
  } else {
    section.value =
      tool.value === 'trigger' ? 'actuation' : tool.value === 'behavior' ? 'advanced' : 'keys';
    view.value = 'layout';
  }
});
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
const colorSource = ref<'preview' | 'assigned' | 'live'>('preview');
const previewEnabled = computed(() => page.value === 'lighting' && colorSource.value === 'preview');
const lightingPreview = useLightingPreview(light, draft, previewEnabled);
const lightCategory = ref('Постоянный свет');
const visibleEffects = computed(
  () => effectGroups.find((g) => g.name === lightCategory.value)!.modes,
);
const performance = ref<PerformanceSettings>({
  reportRate: 6,
  topDeadZoneUm: 0,
  bottomDeadZoneUm: 0,
  keyDelay: 0,
});
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
          `${layer === 'function' ? 'Fn + ' : ''}${label}: ${t(bindingName(a[layer], label))} → ${t(bindingName(b[layer], label))}`,
        );
    if (changed(a.actuation, b.actuation))
      lines.push(
        `${label}: ${t('Ход')} ${(a.actuation.triggerUm / 1000).toFixed(2)} → ${(b.actuation.triggerUm / 1000).toFixed(2)} ${t('мм')}; RT ${b.actuation.rapidTrigger ? `${(b.actuation.pressUm / 1000).toFixed(2)} / ${(b.actuation.releaseUm / 1000).toFixed(2)} мм` : t('выключен')}`,
      );
    if (changed(a.color, b.color))
      lines.push(`${label}: ${t('Цвет клавиш')} ${hexColor(a.color)} → ${hexColor(b.color)}`);
  }
  if (changed(before.lighting, after.lighting))
    lines.push(
      `${t('Свет')}: ${t(effects[before.lighting.mode] ?? String(before.lighting.mode))} → ${t(effects[after.lighting.mode] ?? String(after.lighting.mode))}; ${t('Яркость')} ${before.lighting.brightness} → ${after.lighting.brightness}; ${t('Скорость')} ${before.lighting.speed} → ${after.lighting.speed}; ${t('Цвет эффекта')} ${hexColor(after.lighting.color)}`,
    );
  if (changed(before.performance, after.performance))
    lines.push(
      `${t('Частота опроса')} ${{ 3: 1000, 5: 4000, 6: 8000 }[after.performance.reportRate] ?? '?'} Hz; ${t('Свободный ход сверху')} / ${t('Свободный ход снизу')} ${(after.performance.topDeadZoneUm / 1000).toFixed(2)} / ${(after.performance.bottomDeadZoneUm / 1000).toFixed(2)} мм`,
    );
  if (changed(before.macros, after.macros))
    lines.push(
      `${t('Макросы')}: ${before.macros.length} → ${after.macros.length}; ${t('действий')}: ${after.macros.reduce((n, m) => n + m.steps.length, 0)}`,
    );
  const dks = after.dks.filter((d) => changed(before.dks[d.index], d));
  if (dks.length) lines.push(`DKS: ${dks.map((d) => d.index + 1).join(', ')}`);
  return lines;
});
const colorHex = computed({
  get: () => hexColor(light.value.color),
  set: (value: string) => {
    if (/^#[0-9a-f]{6}$/i.test(value)) light.value.color = parseColor(value);
  },
});

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
watch([page, tool, () => selected.value[0]], async () => {
  await nextTick();
  document.querySelector('.editor-scroll')?.scrollTo({ top: 0 });
});
function select(slot: number, toggle: boolean, paint = false) {
  if (previewEnabled.value && light.value.mode !== 3) lightingPreview.pulse(slot);
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
  colorSource.value = 'assigned';
}
function loadProfile(profile: LocalProfile) {
  try {
    if (snapshot.value) {
      workspace.stage(...profileEdits(draft.value!, profile.snapshot), {
        kind: 'automation',
        value: profile.automation ?? defaultAutomation(),
      });
      notice.value = 'Профиль добавлен в черновик. Устройство ещё не изменено.';
    } else {
      snapshot.value = clone(profile.snapshot);
      workspace.stage({ kind: 'automation', value: profile.automation ?? defaultAutomation() });
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
  game: 'Клавиатура',
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
    <header class="app-header">
      <strong class="app-brand">io <span>Type 84</span></strong>
      <nav class="main-nav" :aria-label="t('Разделы')">
        <button
          v-for="item in [
            { id: 'keyboard', label: 'Клавиатура', icon: 'keyboard' },
            { id: 'lighting', label: 'Свет', icon: 'light' },
            { id: 'macros', label: 'Каталог', icon: 'macro' },
            { id: 'profiles', label: 'Профили', icon: 'profile' },
            { id: 'settings', label: 'Параметры', icon: 'settings' },
          ]"
          :key="item.id"
          :class="{ active: page === item.id }"
          :aria-current="page === item.id ? 'page' : undefined"
          @click="page = item.id"
        >
          <Icon :name="item.icon" /><span>{{ t(item.label) }}</span>
        </button>
      </nav>
      <button
        v-if="live.rulesEnabled"
        class="runtime-stop"
        @click="workspace.setActionsEnabled(false)"
      >
        {{ t('Остановить жесты') }}
      </button>
      <button
        class="connection-button"
        :class="{ connected }"
        :disabled="!native || !!busy"
        @click="connected ? workspace.disconnect() : workspace.connect()"
      >
        <span class="status-dot"></span
        >{{ t(connected ? 'Подключено' : native ? 'Подключить' : 'Предпросмотр') }}
      </button>
    </header>
    <div v-if="error" class="message error-message" role="alert">
      <span>{{ t(error) }}</span
      ><button :aria-label="t('Закрыть')" @click="error = ''">×</button>
    </div>
    <main
      class="main-content"
      :class="{
        'has-keyboard': page === 'keyboard' || page === 'lighting',
        'keyboard-workspace': page === 'keyboard' || page === 'lighting',
        'lighting-page': page === 'lighting',
      }"
    >
      <template v-if="page === 'keyboard' || page === 'lighting'">
        <section class="keyboard-area">
          <div class="keyboard-toolbar">
            <div class="segmented" v-if="page === 'keyboard'">
              <button
                :class="{ active: !functionLayer }"
                :aria-pressed="!functionLayer"
                @click="functionLayer = false"
              >
                {{ t('Основной') }}</button
              ><button
                :class="{ active: functionLayer }"
                :aria-pressed="functionLayer"
                @click="functionLayer = true"
              >
                Fn
              </button>
            </div>
            <select v-else v-model="colorSource" :aria-label="t('Отображение света')">
              <option value="preview">{{ t('Предпросмотр эффекта') }}</option>
              <option value="assigned">{{ t('Заданные цвета') }}</option>
              <option value="live">{{ t('Текущие цвета') }}</option>
            </select>
            <form class="key-search" @submit.prevent="findKey">
              <Icon name="search" /><input
                v-model="search"
                :placeholder="t('Найти клавишу')"
                :aria-label="t('Найти клавишу')"
                type="search"
              />
            </form>
            <div class="group-actions">
              <button
                class="text-button"
                :class="{ chosen: multi }"
                :aria-pressed="multi"
                @click="multi = !multi"
              >
                {{ t('Группа') }}</button
              ><button class="text-button" @click="group('wasd')">WASD</button
              ><button class="text-button" @click="group('all')">{{ t('Все') }}</button
              ><button
                class="text-button clear-selection"
                :disabled="!selected.length"
                :title="t('Снять выделение') + ' · Esc'"
                @click="selected = []"
              >
                {{ t('Снять выделение') }}
              </button>
            </div>
            <button
              class="monitor-toggle"
              :class="{ active: live.active }"
              :aria-pressed="live.active"
              :disabled="!connected || !!busy"
              @click="workspace.monitor()"
            >
              <Icon name="monitor" />{{ t(live.active ? 'Остановить' : 'Наблюдать') }}
            </button>
          </div>
          <KeyboardPreview
            :selected="selected"
            :snapshot="draft"
            :live="displayLive"
            :view="view"
            :function-layer="functionLayer"
            :multi="multi"
            :actual-colors="page === 'lighting' && colorSource === 'live'"
            :preview-colors="lightingPreview.colors.value"
            @pointerup="light.mode === 3 && selected.forEach(lightingPreview.pulse)"
            :automation="automation"
            @select="select"
          />
          <p
            v-if="page === 'lighting' && colorSource === 'live' && !live.colors.length"
            class="inline-notice"
          >
            {{
              t(
                'Клавиатура пока не отдаёт подтверждённые текущие цвета. Штрих означает неизвестный цвет.',
              )
            }}
          </p>
          <div v-if="previewEnabled" class="preview-caption">
            <span
              >{{ t('Визуальная модель. Рисунок на устройстве может отличаться.')
              }}<small v-if="reactiveModes.includes(light.mode)">{{
                t('Нажмите клавишу на макете для проверки отклика.')
              }}</small></span
            >
            <button
              class="text-button"
              @click="lightingPreview.playing.value = !lightingPreview.playing.value"
            >
              {{ t(lightingPreview.playing.value ? 'Пауза' : 'Воспроизвести') }}
            </button>
          </div>
          <MonitorStrip
            v-if="live.active || live.history.length"
            :live="displayLive"
            :selected="selected"
            @select="select($event, false)"
            @clear="workspace.clearHistory()"
            @capacity="workspace.setHistoryCapacity"
          />
          <KeyInspection
            :snapshot="draft"
            :selected="selected"
            :live="displayLive"
            :function-layer="functionLayer"
          />
        </section>
        <div class="editor-scroll" tabindex="-1">
          <div v-if="!draft" class="empty-workspace">
            <Icon name="keyboard" />
            <h2>{{ t('Подключите клавиатуру') }}</h2>
            <p>{{ t('Прочитайте её настройки или откройте пример, чтобы изучить редактор.') }}</p>
            <div>
              <button class="primary" :disabled="!native || !!busy" @click="workspace.connect()">
                {{ t('Подключить') }}</button
              ><button class="secondary" @click="openExample">{{ t('Открыть пример') }}</button>
            </div>
          </div>
          <template v-else-if="page === 'keyboard'">
            <nav class="editor-tabs" :aria-label="t('Настройка клавиш')">
              <button
                v-for="tab in [
                  { id: 'assignment', label: 'Действия' },
                  { id: 'trigger', label: 'Срабатывание' },
                  { id: 'behavior', label: 'Поведение' },
                ]"
                :key="tab.id"
                :class="{ active: tool === tab.id }"
                @click="tool = tab.id"
              >
                {{ t(tab.label) }}</button
              ><span class="selection-summary">{{
                t(
                  String(
                    selected.length === 1
                      ? keyLabel(selected[0]!)
                      : selected.length + ' ' + t('выбрано'),
                  ),
                )
              }}</span>
            </nav>
            <div class="editor-columns" :class="{ 'behavior-columns': tool === 'behavior' }">
              <Inspector
                :snapshot="draft"
                :selected="selected"
                :section="section"
                :function-layer="functionLayer"
                :disabled="!!busy"
                :live="displayLive"
                @stage="workspace.stage"
              />
              <div v-if="tool === 'assignment' && selected.length > 0" class="companion-editor">
                <DepthActions
                  :selected="selected"
                  :profile="automation"
                  :live="displayLive"
                  :disabled="!!busy || functionLayer"
                  @save="workspace.stage({ kind: 'automation', value: $event })"
                  @catalog="
                    page = 'macros';
                    catalogTab = 'computer';
                  "
                />
                <div class="software-status">
                  <label class="switch-line"
                    ><span
                      >{{ t('Действия компьютера')
                      }}<small>{{
                        t('Работают при открытом приложении и наблюдении')
                      }}</small></span
                    ><input
                      type="checkbox"
                      role="switch"
                      :checked="live.rulesEnabled"
                      :disabled="
                        !native ||
                        !connected ||
                        !workspace.appliedAutomation.value.gestures.length ||
                        !!busy
                      "
                      @change="
                        workspace.setActionsEnabled(($event.target as HTMLInputElement).checked)
                      "
                  /></label>
                  <p v-if="functionLayer" class="hint">
                    {{
                      t(
                        'Условия глубины используют физическую клавишу независимо от Fn. Настройте их в основном слое.',
                      )
                    }}
                  </p>
                  <p v-if="live.ruleError" class="hint" role="status">
                    {{ t(live.ruleError) }}
                  </p>
                  <small v-if="live.rulesEnabled"
                    >{{ t('Выполнено') }}: {{ live.ruleFirings }}</small
                  >
                </div>
              </div>
              <div v-else-if="tool === 'trigger' && selected.length" class="trigger-guide">
                <h3>{{ t('Проверьте нажатие') }}</h3>
                <p>
                  {{
                    t(
                      'Зелёная отметка показывает живой ход, синяя — точку срабатывания. Включите наблюдение и нажмите выбранную клавишу.',
                    )
                  }}
                </p>
                <div class="travel-illustration" aria-hidden="true">
                  <div class="switch-cap"></div>
                  <div class="switch-stem"></div>
                  <div class="switch-base"></div>
                </div>
                <p class="hint">
                  {{
                    t(
                      'Измеряется глубина хода, а не сила в граммах. Пропавший сигнал отображается как неизвестный.',
                    )
                  }}
                </p>
              </div>
              <DksEditor
                v-else-if="tool === 'behavior'"
                :snapshot="draft"
                :selected="selected"
                :function-layer="functionLayer"
                :disabled="!!busy"
                @stage="workspace.stage"
              />
            </div>
          </template>
          <template v-else>
            <div class="lighting-workspace">
              <section class="light-effects">
                <div class="effect-categories">
                  <button
                    v-for="category in effectGroups"
                    :key="category.name"
                    :class="{ active: lightCategory === category.name }"
                    @click="lightCategory = category.name"
                  >
                    {{ t(category.name) }}
                  </button>
                </div>
                <div class="effect-grid">
                  <button
                    v-for="index in visibleEffects"
                    :key="index"
                    :class="{ chosen: light.mode === index }"
                    @click="
                      light.mode = index;
                      colorSource = 'preview';
                    "
                  >
                    {{ t(effects[index]!) }}
                  </button>
                </div>
                <p class="effect-description">{{ t(effectDescriptions[light.mode] ?? '') }}</p>
              </section>
              <section class="light-adjustments">
                <label
                  >{{ t('Яркость')
                  }}<input v-model.number="light.brightness" type="range" min="0" max="5" /></label
                ><label
                  >{{ t('Скорость')
                  }}<input v-model.number="light.speed" type="range" min="0" max="5" /></label
                ><label
                  >{{ t('Цвет эффекта')
                  }}<select v-model.number="light.colorMode">
                    <option :value="0">{{ t('Один цвет') }}</option>
                    <option :value="1">RGB</option>
                  </select></label
                >
                <div v-if="light.colorMode === 0" class="color-input">
                  <input v-model="colorHex" type="color" :aria-label="t('Цвет эффекта')" /><span>{{
                    colorHex
                  }}</span>
                </div>
                <label
                  >{{ t('Направление')
                  }}<select v-model.number="light.direction">
                    <option :value="0">{{ t('Вперёд') }}</option>
                    <option :value="1">{{ t('Назад') }}</option>
                  </select></label
                ><button
                  class="primary"
                  :disabled="!!busy"
                  @click="workspace.stage({ kind: 'lighting', value: clone(light) })"
                >
                  {{ t('Добавить в черновик') }}
                </button>
              </section>
              <section class="paint-editor">
                <h2>{{ t('Цвет клавиш') }}</h2>
                <Inspector
                  :snapshot="draft"
                  :selected="selected"
                  section="lighting"
                  :function-layer="functionLayer"
                  :disabled="!!busy"
                  :live="displayLive"
                  @stage="workspace.stage"
                />
                <details>
                  <summary>{{ t('Градиент') }}</summary>
                  <div
                    class="gradient-preview"
                    :style="{
                      background: `linear-gradient(${gradientDirection === 'horizontal' ? '90' : '180'}deg,${gradientStart},${gradientEnd})`,
                    }"
                  ></div>
                  <div class="gradient-controls">
                    <label>{{ t('Начало') }}<input v-model="gradientStart" type="color" /></label
                    ><label>{{ t('Конец') }}<input v-model="gradientEnd" type="color" /></label
                    ><select v-model="gradientDirection" :aria-label="t('Направление')">
                      <option value="horizontal">{{ t('Слева направо') }}</option>
                      <option value="vertical">{{ t('Сверху вниз') }}</option>
                    </select>
                  </div>
                  <button class="secondary full" :disabled="!!busy" @click="gradient">
                    {{ t('Создать градиент') }}
                  </button>
                </details>
                <details>
                  <summary>{{ t('LED-панель') }}</summary>
                  <p class="hint">
                    {{
                      t(
                        'Fn+G — глубина RT, Fn+K — наложение, Fn+L — эффект. Их можно переназначить в действиях клавиатуры. Прямой редактор панели появится после проверки протокола.',
                      )
                    }}
                  </p>
                </details>
              </section>
            </div>
          </template>
        </div>
      </template>
      <div v-else class="page-scroll">
        <template v-if="page === 'macros'">
          <nav class="editor-tabs">
            <button :class="{ active: catalogTab === 'computer' }" @click="catalogTab = 'computer'">
              {{ t('Действия компьютера') }}</button
            ><button
              :class="{ active: catalogTab === 'keyboard' }"
              @click="catalogTab = 'keyboard'"
            >
              {{ t('Макросы клавиатуры') }}
            </button>
          </nav>
          <ActionCatalog
            v-if="draft && catalogTab === 'computer'"
            :profile="automation"
            :disabled="!!busy"
            @save="workspace.stage({ kind: 'automation', value: $event })"
          />
          <MacroEditor
            v-else-if="draft"
            :snapshot="draft"
            :disabled="!!busy"
            @stage="workspace.stage"
          />
          <div v-else class="empty-workspace">
            <h2>{{ t('Сначала откройте профиль') }}</h2>
            <button
              class="secondary"
              @click="
                openExample();
                page = 'macros';
              "
            >
              {{ t('Открыть пример') }}
            </button>
          </div>
        </template>
        <section v-if="page === 'settings'" class="preferences-panel">
          <h2>{{ t('Приложение') }}</h2>
          <label
            >{{ t('Оформление')
            }}<select v-model="theme" :aria-label="t('Оформление')">
              <option value="system">{{ t('Как в системе') }}</option>
              <option value="light">{{ t('Светлое') }}</option>
              <option value="dark">{{ t('Тёмное') }}</option>
            </select></label
          ><label
            >{{ t('Язык')
            }}<select v-model="locale" :aria-label="t('Язык')">
              <option value="ru">{{ t('Русский') }}</option>
              <option value="en">English</option>
            </select></label
          >
        </section>
        <section v-if="page === 'profiles'" class="surface editor-panel">
          <div class="panel-heading">
            <div>
              <h2>{{ t('Профили') }}</h2>
            </div>
            <span class="tag">{{ profiles.length }}{{ t('профилей') }}</span>
          </div>
          <div class="profile-create">
            <input
              v-model="profileName"
              maxlength="100"
              :placeholder="t('Название профиля')"
              :aria-label="t('Название профиля')"
            /><button
              class="primary"
              :disabled="!draft"
              @click="workspace.saveProfile(profileName)"
            >
              {{ t('Сохранить текущий') }}</button
            ><button
              class="secondary"
              :disabled="!draft"
              @click="workspace.exportProfile(profileName)"
            >
              {{ t('Экспорт JSON') }}</button
            ><label class="secondary file-button"
              >{{ t('Импорт')
              }}<input type="file" accept=".json,application/json" @change="importProfile"
            /></label>
          </div>
          <p class="hint">
            {{
              t(
                'Ctrl+S сохраняет локальный профиль. Применение к клавиатуре выполняется отдельно. Формат Поддерживаются собственный JSON и проверенные поля экспорта IO Vision. Неоднозначные поля сайта сохраняют значения исходного снимка.',
              )
            }}
          </p>
          <div v-if="profiles.length" class="profile-grid">
            <article v-for="profile in profiles" :key="profile.name" class="profile-card">
              <span>▤</span>
              <h3>{{ profile.name }}</h3>
              <p>{{ new Date(profile.savedAt).toLocaleString(locale) }}</p>
              <div>
                <button class="secondary" @click="loadProfile(profile)">
                  {{ t('Открыть в черновике') }}</button
                ><button class="text-button" @click="workspace.deleteProfile(profile.name)">
                  {{ t('Удалить') }}
                </button>
              </div>
            </article>
          </div>
          <p v-else class="empty-inline">
            {{ t('Сохраните первую настройку, чтобы быстро вернуться к ней.') }}
          </p>
        </section>
        <section v-if="page === 'settings'" class="settings-grid">
          <div class="surface editor-panel">
            <h2>{{ t('Клавиатура') }}</h2>
            <fieldset :disabled="!draft || !!busy">
              <label
                >{{ t('Частота опроса')
                }}<select v-model.number="performance.reportRate">
                  <option :value="3">{{ t('1000 Гц') }}</option>
                  <option :value="5">{{ t('4000 Гц') }}</option>
                  <option :value="6">{{ t('8000 Гц') }}</option>
                </select></label
              >
              <TravelSlider
                v-model="performance.topDeadZoneUm"
                :label="t('Свободный ход сверху')"
                :min="0"
                :max="500"
                :low="t('Без зоны')"
                :high="t('Больше')"
              /><TravelSlider
                v-model="performance.bottomDeadZoneUm"
                :label="t('Свободный ход снизу')"
                :min="0"
                :max="500"
                :low="t('Без зоны')"
                :high="t('Больше')"
              />
              <button
                class="primary"
                @click="workspace.stage({ kind: 'performance', value: clone(performance) })"
              >
                {{ t('Добавить в черновик') }}
              </button>
            </fieldset>
          </div>
          <div class="surface editor-panel">
            <h2>{{ t('Устройство') }}</h2>
            <dl class="device-facts">
              <dt>{{ t('Модель') }}</dt>
              <dd>IO White</dd>
              <dt>{{ t('Прошивка') }}</dt>
              <dd>{{ snapshot?.identity.firmware ?? '—' }}</dd>
              <dt>{{ t('Протокол') }}</dt>
              <dd>io_vision_v0</dd>
              <dt>{{ t('Текущие RGB') }}</dt>
              <dd>
                {{ t(String(live.colors.length ? 'Получены от устройства' : 'Не подтверждены')) }}
              </dd>
              <dt>{{ t('Пакеты хода') }}</dt>
              <dd>{{ live.packets.toLocaleString('ru-RU') }}</dd>
            </dl>
            <p class="hint">
              {{
                t(
                  'Перед записью сохраняется резервный снимок в каталоге приложения. Восстановление сначала покажет список блоков.',
                )
              }}
            </p>
            <button
              class="secondary"
              :disabled="!connected || !!busy"
              @click="workspace.recovery()"
            >
              {{ t('Подготовить восстановление') }}
            </button>
          </div>
        </section>
        <ul v-if="importNotes.length" class="import-notes">
          <li v-for="note in importNotes" :key="note">{{ t(note) }}</li>
        </ul>
      </div>
    </main>
    <footer class="draft-bar">
      <span class="draft-summary" role="status">{{
        t(
          String(
            busy
              ? t(busy)
              : edits.length
                ? edits.length + ' ' + t('изменений в черновике')
                : t(notice || (draft ? 'Без изменений' : 'Нет открытого профиля')),
          ),
        )
      }}</span>
      <div class="draft-actions">
        <button
          class="icon-button"
          :disabled="!edits.length || !!busy"
          :title="t('Отменить · Ctrl+Z')"
          :aria-label="t('Отменить')"
          @click="workspace.undo()"
        >
          <Icon name="undo" /></button
        ><button
          class="icon-button"
          :disabled="!redo.length || !!busy"
          :title="t('Повторить · Ctrl+Shift+Z')"
          :aria-label="t('Повторить')"
          @click="workspace.redoEdit()"
        >
          <Icon name="redo" /></button
        ><button
          class="text-button"
          :disabled="!edits.length || !!busy"
          @click="confirmDiscard = true"
        >
          {{ t('Сбросить') }}</button
        ><button
          class="secondary"
          :disabled="!draft || !!busy"
          @click="workspace.saveProfile(profileName)"
        >
          {{ t('Сохранить профиль') }}</button
        ><button
          class="primary"
          :disabled="(!connected && hardwareEdits.length > 0) || !edits.length || !!busy"
          @click="workspace.prepare()"
        >
          {{ t('Применить') }}
        </button>
      </div>
    </footer>
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
        <template v-if="preview">
          <h2>{{ t('Проверьте изменения') }}</h2>
          <p>
            {{
              t(
                'Исходное состояние будет проверено ещё раз. Затем приложение сохранит резервный снимок и сверит результат чтением.',
              )
            }}
          </p>
          <div
            v-if="!recoveryPreview && (automationDirty || automation.gestures.length)"
            class="automation-preview"
          >
            <b>{{ t('На компьютере') }}</b>
            <p>
              {{ automation.actions.length }} {{ t('действий') }} ·
              {{ automation.gestures.length }} {{ t('жестов') }}
            </p>
            <ul>
              <li v-for="r in automation.gestures" :key="r.id">
                {{ r.slots.map(keyLabel).join(' + ') }} · {{ r.thresholdUm / 1000 }} {{ t('мм')
                }}{{ r.holdMs ? ' · ' + r.holdMs / 1000 + ' ' + t('с') : '' }} →
                {{ t(automation.actions.find((a) => a.id === r.actionId)?.name ?? '') }}
              </li>
            </ul>
            <label v-if="native && connected && automation.gestures.length" class="switch-line"
              ><span>{{ t('Включить жесты после применения') }}</span
              ><input v-model="startActions" type="checkbox"
            /></label>
            <p class="hint">
              {{
                t(
                  'Действия компьютера сохраняются в профиле приложения. Они не записываются в прошивку.',
                )
              }}
            </p>
          </div>
          <ul class="change-list">
            <li v-for="change in preview.changes" :key="change.block">
              <b>{{ blockNames[change.block] ?? change.block }}</b
              ><span>{{ t('Будет обновлён') }}</span>
            </li>
          </ul>
          <ul
            v-if="previewDetails.length"
            class="semantic-changes"
            tabindex="0"
            :aria-label="t('Подробности изменений')"
          >
            <li v-for="(line, index) in previewDetails" :key="index">{{ line }}</li>
          </ul>
          <p v-if="recoveryPreview" class="hint">
            {{ t('Восстанавливаются исходные данные этих блоков из последней резервной копии.') }}
          </p>
          <p v-if="!preview.changes.length && !automationDirty">
            {{ t('Отличий от устройства нет.') }}
          </p>
          <p v-if="error" class="dialog-error" role="alert">{{ error }}</p>
          <div class="dialog-actions">
            <button class="secondary" :disabled="!!busy" @click="preview = null">
              {{ t('Вернуться') }}</button
            ><button
              class="primary"
              :disabled="!!busy || (!preview.changes.length && !automationDirty)"
              @click="workspace.apply()"
            >
              {{
                t(
                  String(
                    busy ||
                      (preview.changes.length ? 'Применить изменения' : 'Сохранить на компьютере'),
                  ),
                )
              }}
            </button>
          </div></template
        ><template v-else
          ><h2>
            {{ t(String(confirmRefresh ? 'Перечитать настройки?' : 'Отменить весь черновик?')) }}
          </h2>
          <p>
            {{
              t('Неприменённые изменения будут удалены. Настройки клавиатуры останутся прежними.')
            }}
          </p>
          <div class="dialog-actions">
            <button
              class="secondary"
              @click="
                confirmDiscard = false;
                confirmRefresh = false;
              "
            >
              {{ t('Оставить') }}</button
            ><button
              class="primary"
              @click="
                confirmRefresh ? workspace.refresh() : workspace.discard();
                confirmDiscard = false;
                confirmRefresh = false;
              "
            >
              {{ t(String(confirmRefresh ? 'Перечитать' : 'Отменить черновик')) }}
            </button>
          </div></template
        >
      </section>
    </div>
  </div>
</template>
