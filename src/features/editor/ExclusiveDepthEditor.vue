<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type { AutomationProfile, MonitorFrame } from '../../shared/contracts/generated';
import { pageDepthChoice } from './computer-rules';
import { sampleDepth } from './telemetry';
import { t } from '../../shared/ui/preferences';
import TravelSlider from '../../shared/ui/TravelSlider.vue';
const props = defineProps<{
  slot: number;
  profile: AutomationProfile;
  live: MonitorFrame;
  disabled: boolean;
}>();
const emit = defineEmits<{ save: [profile: AutomationProfile]; catalog: [] }>();
const current = computed(() => props.profile.depthChoices.find((r) => r.slot === props.slot));
const light = ref(600),
  deep = ref(3000),
  lightAction = ref(''),
  deepAction = ref('');
watch(
  [() => props.slot, current],
  () => {
    light.value = current.value?.lightUm ?? 600;
    deep.value = current.value?.deepUm ?? 3000;
    lightAction.value = current.value?.lightActionId ?? '';
    deepAction.value =
      current.value?.deepActionId ?? (props.slot === 105 ? 'volumeUp' : 'volumeDown');
  },
  { immediate: true },
);
watch(light, (value) => {
  if (deep.value < value + 100) deep.value = value + 100;
});
const depth = computed(() => sampleDepth(props.live, props.slot));
const conflicts = computed(
  () => props.profile.gestures.filter((g) => g.slots.includes(props.slot)).length,
);
function save() {
  emit(
    'save',
    pageDepthChoice(
      props.profile,
      props.slot,
      deepAction.value,
      deep.value,
      lightAction.value || undefined,
      light.value,
    ),
  );
}
function remove() {
  emit('save', {
    ...props.profile,
    depthChoices: props.profile.depthChoices.filter((r) => r.slot !== props.slot),
  });
}
</script>
<template>
  <fieldset class="exclusive-depth" :disabled="disabled">
    <div class="choice-branch">
      <label
        >{{ t('Лёгкое нажатие') }}
        <select v-model="lightAction" :aria-label="t('Действие лёгкого нажатия')">
          <option value="">{{ slot === 105 ? 'Page Up' : 'Page Down' }}</option>
          <option v-for="a in profile.actions" :key="a.id" :value="a.id">{{ t(a.name) }}</option>
        </select>
      </label>
      <p class="hint">{{ t('При отпускании, если не достигнут глубокий порог.') }}</p>
    </div>
    <TravelSlider
      v-model="deep"
      label="Глубокое нажатие"
      :min="light + 100"
      :max="3200"
      :live="depth"
    />
    <label
      >{{ t('Действие глубокого нажатия') }}
      <select v-model="deepAction" :aria-label="t('Действие глубокого нажатия')">
        <option v-for="a in profile.actions" :key="a.id" :value="a.id">{{ t(a.name) }}</option>
      </select>
    </label>
    <p class="hint">
      {{ t('Сразу на пороге. Лёгкое действие отменяется до следующего нажатия.') }}
    </p>
    <details>
      <summary>{{ t('Чувствительность лёгкого нажатия') }}</summary>
      <TravelSlider v-model="light" label="Начало нажатия" :min="300" :max="3000" :live="depth" />
    </details>
    <p v-if="conflicts" class="hint">
      {{ t('Заменит добавочные жесты с этой клавишей:') }} {{ conflicts }}
    </p>
    <div class="panel-actions">
      <button class="text-button" @click="emit('catalog')">{{ t('Каталог действий') }}</button>
      <button
        class="primary"
        :disabled="!profile.actions.some((a) => a.id === deepAction)"
        @click="save"
      >
        {{ t('Сохранить действие') }}
      </button>
    </div>
    <button v-if="current" class="text-button" @click="remove">
      {{ t('Удалить выбор по глубине') }}
    </button>
    <p class="hint">
      {{
        t('Работает в приложении, в том числе с Fn. При остановке возвращаются прежние назначения.')
      }}
    </p>
  </fieldset>
</template>
