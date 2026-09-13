<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type { AutomationProfile, MonitorFrame } from '../../shared/contracts/generated';
import { actionLabel } from './computer-rules';
import { sampleDepth } from './telemetry';
import { t, mm } from '../../shared/ui/preferences';
import TravelSlider from '../../shared/ui/TravelSlider.vue';
const props = defineProps<{
  selected: number[];
  profile: AutomationProfile;
  live: MonitorFrame;
  disabled: boolean;
}>();
const emit = defineEmits<{ save: [profile: AutomationProfile]; catalog: [] }>();
const threshold = ref(2400),
  hold = ref(1000),
  held = ref(false),
  actionId = ref('volumeUp'),
  editing = ref<string | null>(null);
const matches = computed(() =>
  props.profile.gestures.filter(
    (r) =>
      r.slots.length === props.selected.length && r.slots.every((s) => props.selected.includes(s)),
  ),
);
const current = computed(() => props.profile.gestures.find((r) => r.id === editing.value));
function reset() {
  editing.value = null;
  threshold.value = 2400;
  held.value = false;
  hold.value = 1000;
  actionId.value = props.selected[0] === 108 ? 'volumeDown' : (props.profile.actions[0]?.id ?? '');
}
watch(() => props.selected.join(','), reset, { immediate: true });
function edit(id: string) {
  editing.value = id;
  const r = current.value;
  if (r) {
    threshold.value = r.thresholdUm;
    held.value = r.holdMs > 0;
    hold.value = r.holdMs || 1000;
    actionId.value = r.actionId;
  }
}
watch(
  () => props.profile,
  () => {
    if (editing.value && !current.value) reset();
  },
);
const depth = computed(() => sampleDepth(props.live, props.selected[0] ?? -1));
function save() {
  if (!props.selected.length || props.selected.length > 8 || !actionId.value) return;
  const next = JSON.parse(JSON.stringify(props.profile)) as AutomationProfile;
  next.gestures = next.gestures.filter((r) => r.id !== editing.value);
  const id = editing.value ?? crypto.randomUUID();
  next.gestures.push({
    id,
    slots: [...props.selected],
    thresholdUm: threshold.value,
    releaseUm: Math.max(0, threshold.value - 600),
    holdMs: held.value ? hold.value : 0,
    actionId: actionId.value,
  });
  emit('save', next);
  editing.value = id;
}
function remove(id: string) {
  emit('save', { ...props.profile, gestures: props.profile.gestures.filter((r) => r.id !== id) });
  reset();
}
</script>
<template>
  <section class="depth-actions">
    <div class="field-heading">
      <h3>{{ t('Жесты') }}</h3>
      <span class="tag">{{ t('На компьютере') }}</span>
    </div>
    <div v-if="matches.length" class="gesture-list">
      <div v-for="r in matches" :key="r.id" :class="{ chosen: editing === r.id }">
        <button @click="edit(r.id)">
          <span
            >{{ mm(r.thresholdUm) }} {{ t('мм')
            }}{{ r.holdMs ? ' · ' + r.holdMs / 1000 + ' ' + t('с') : '' }}</span
          ><b>{{ t(actionLabel(profile.actions.find((a) => a.id === r.actionId))) }}</b>
        </button>
        <button class="text-button" :aria-label="t('Удалить жест')" @click="remove(r.id)">×</button>
      </div>
      <button class="text-button" @click="reset">+ {{ t('Ещё жест') }}</button>
    </div>
    <fieldset :disabled="disabled || !selected.length || selected.length > 8">
      <p v-if="selected.length > 1" class="hint">{{ t('Все выбранные клавиши одновременно') }}</p>
      <TravelSlider v-model="threshold" label="Сработает на глубине" :min="300" :live="depth" />
      <div class="segmented">
        <button :class="{ active: threshold === 1200 }" @click="threshold = 1200">
          {{ t('Неполное') }}</button
        ><button :class="{ active: threshold === 3000 }" @click="threshold = 3000">
          {{ t('Полное') }}
        </button>
      </div>
      <label class="switch-line"
        ><span>{{ t('Удерживать') }}</span
        ><input v-model="held" type="checkbox" role="switch"
      /></label>
      <label v-if="held"
        >{{ t('Длительность') }} · {{ hold / 1000 }} {{ t('с')
        }}<input
          v-model.number="hold"
          type="range"
          min="250"
          max="3000"
          step="250"
          :aria-label="t('Длительность')"
      /></label>
      <label
        >{{ t('Действие')
        }}<select v-model="actionId" :aria-label="t('Действие жеста')">
          <option v-for="a in profile.actions" :key="a.id" :value="a.id">{{ t(a.name) }}</option>
        </select></label
      >
      <div class="panel-actions">
        <button class="text-button" @click="emit('catalog')">{{ t('Каталог действий') }}</button
        ><button class="primary" :disabled="!actionId" @click="save">
          {{ t('Сохранить действие') }}
        </button>
      </div>
    </fieldset>
    <p class="hint">
      {{ t('Один раз за нажатие. Обычный ввод клавиш сохраняется. Требуются свежие данные хода.') }}
    </p>
  </section>
</template>
