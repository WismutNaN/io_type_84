<script setup lang="ts">
import { ref } from 'vue';
import { keyboardKeys, keyboardBounds } from './layout';

const selected = ref<number | null>(null);
</script>

<template>
  <div class="keyboard-preview">
    <div class="keyboard" aria-label="Схема IO Type 84: выбор клавиши только в макете">
      <div
        class="keyboard-board"
        :style="{ aspectRatio: `${keyboardBounds.width} / ${keyboardBounds.height}` }"
      >
        <button
          v-for="key in keyboardKeys"
          :key="key.slot"
          class="keycap"
          :class="{ selected: selected === key.slot }"
          :style="{
            left: `${(key.x / keyboardBounds.width) * 100}%`,
            top: `${(key.y / keyboardBounds.height) * 100}%`,
            width: `${(key.width / keyboardBounds.width) * 100}%`,
            height: `${(key.height / keyboardBounds.height) * 100}%`,
          }"
          :aria-label="`Клавиша ${key.label}`"
          :aria-pressed="selected === key.slot"
          @click="selected = selected === key.slot ? null : key.slot"
        >
          {{ key.label === 'Space' ? '' : key.label
          }}<span v-if="key.label === 'Space'" class="space-mark"></span>
        </button>
      </div>
    </div>
    <div class="preview-caption">
      <span><span class="tiny-dot"></span> Схема модели · 84 клавиши</span>
      <span aria-live="polite">{{
        selected === null ? 'Выберите клавишу на схеме' : 'Клавиша выделена в макете'
      }}</span>
    </div>
  </div>
</template>
