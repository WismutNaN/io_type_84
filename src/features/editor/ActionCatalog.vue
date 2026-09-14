<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type {
  ActionDefinition,
  AutomationProfile,
  ActionCommand,
} from '../../shared/contracts/generated';
import { readAutomation, emptyPlatforms } from './computer-rules';
import { clone } from './model';
import { t } from '../../shared/ui/preferences';
import CommandEditor from './CommandEditor.vue';
const props = defineProps<{ profile: AutomationProfile; disabled: boolean }>();
const emit = defineEmits<{ save: [profile: AutomationProfile] }>();
const platform = ref<'windows' | 'linux' | 'macos'>('windows');
const selected = ref(props.profile.actions[0]?.id ?? ''),
  search = ref(''),
  error = ref('');
const current = ref<ActionDefinition | null>(null);
const visible = computed(() =>
  props.profile.actions.filter((a) => t(a.name).toLowerCase().includes(search.value.toLowerCase())),
);
const uses = computed(
  () =>
    props.profile.gestures.filter((g) => g.actionId === selected.value).length +
    props.profile.depthChoices.filter(
      (r) => r.lightActionId === selected.value || r.deepActionId === selected.value,
    ).length,
);
watch(
  [selected, () => props.profile],
  () => {
    if (!selected.value) return;
    current.value = clone(props.profile.actions.find((a) => a.id === selected.value) ?? null);
    error.value = '';
  },
  { immediate: true },
);
function create() {
  selected.value = '';
  current.value = {
    id: crypto.randomUUID(),
    name: '',
    platformCommands: emptyPlatforms(),
    command: { kind: 'media', action: 'volumeUp' },
  };
}
function save() {
  if (!current.value) return;
  const next = {
    ...props.profile,
    actions: [
      ...props.profile.actions.filter((a) => a.id !== current.value!.id),
      clone(current.value),
    ],
  };
  try {
    readAutomation(next);
    emit('save', next);
    selected.value = current.value.id;
  } catch (e) {
    error.value = String(e instanceof Error ? e.message : e);
  }
}
function remove() {
  if (uses.value || !selected.value) return;
  emit('save', {
    ...props.profile,
    actions: props.profile.actions.filter((a) => a.id !== selected.value),
  });
  selected.value = props.profile.actions.find((a) => a.id !== selected.value)?.id ?? '';
  if (!selected.value) current.value = null;
}
</script>
<template>
  <div class="action-catalog">
    <aside class="catalog-list">
      <input
        v-model="search"
        type="search"
        :placeholder="t('Найти действие')"
        :aria-label="t('Найти действие')"
      /><button class="secondary" :disabled="disabled" @click="create">
        + {{ t('Новое действие') }}</button
      ><button
        v-for="a in visible"
        :key="a.id"
        :class="{ chosen: selected === a.id }"
        @click="selected = a.id"
      >
        <span>{{ t(a.name) }}</span
        ><small>{{
          t(
            a.command.kind === 'macro'
              ? 'Последовательность'
              : a.command.kind === 'text'
                ? 'Текст'
                : a.command.kind === 'application'
                  ? 'Приложение'
                  : a.command.kind === 'media'
                    ? 'Мультимедиа'
                    : 'Клавиша',
          )
        }}</small>
      </button>
    </aside>
    <section v-if="current" class="catalog-detail">
      <fieldset :disabled="disabled">
        <label
          >{{ t('Название действия')
          }}<input
            v-model="current.name"
            :aria-label="t('Название действия')"
            maxlength="100"
            :placeholder="t('Например: переключить окно')" /></label
        ><CommandEditor
          :model-value="current.command"
          @update:model-value="current.command = $event as ActionCommand"
        />
        <details class="platform-variants">
          <summary>{{ t('Варианты для платформ') }}</summary>
          <select v-model="platform" :aria-label="t('Платформа')">
            <option value="windows">Windows</option>
            <option value="linux">Linux</option>
            <option value="macos">macOS</option></select
          ><label class="switch-line"
            ><span>{{ t('Отдельное действие') }}</span
            ><input
              type="checkbox"
              :checked="!!current.platformCommands[platform]"
              @change="
                current.platformCommands[platform] = ($event.target as HTMLInputElement).checked
                  ? clone(current.command)
                  : null
              " /></label
          ><CommandEditor
            v-if="current.platformCommands[platform]"
            :model-value="current.platformCommands[platform]!"
            @update:model-value="current.platformCommands[platform] = $event as ActionCommand"
          />
          <p class="hint">
            {{
              t(
                'Общее действие используется, если вариант не задан. Исполнение Linux и macOS пока не реализовано.',
              )
            }}
          </p>
        </details>
        <p class="hint">
          {{ t('Выполняется в Windows. Назначьте действие жесту на вкладке клавиатуры.') }}
        </p>
        <p v-if="uses" class="hint">
          {{ t('Используется в жестах') }}: {{ uses }}.
          {{ t('Изменение обновит все назначения этого действия.') }}
        </p>
        <p v-if="error" role="alert" class="dialog-error">{{ t(error) }}</p>
        <div class="panel-actions">
          <button
            class="text-button"
            :disabled="!selected || !!uses"
            :title="uses ? t('Сначала удалите связанные жесты') : ''"
            @click="remove"
          >
            {{ t('Удалить') }}</button
          ><button class="primary" :disabled="!current.name.trim()" @click="save">
            {{ t('Добавить в черновик') }}
          </button>
        </div>
      </fieldset>
    </section>
  </div>
</template>
