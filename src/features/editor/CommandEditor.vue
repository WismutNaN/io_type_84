<script setup lang="ts">
import type { ActionCommand, ActionStep } from '../../shared/contracts/generated';
import { computerActions, supportedKey } from './computer-rules';
import { keyChoices } from './model';
import { t } from '../../shared/ui/preferences';
const model = defineModel<ActionCommand | ActionStep>({ required: true });
defineProps<{ step?: boolean }>();
function change(kind: string) {
  model.value =
    kind === 'media'
      ? { kind, action: 'volumeUp' }
      : kind === 'key'
        ? { kind, key: 4, modifiers: 0 }
        : kind === 'text'
          ? { kind, text: '' }
          : kind === 'application'
            ? { kind, application: 'word' }
            : kind === 'delay'
              ? { kind, ms: 500 }
              : { kind: 'macro', steps: [{ kind: 'key', key: 43, modifiers: 4 }] };
}
function move(index: number, delta: number) {
  if (model.value.kind !== 'macro') return;
  const steps = model.value.steps;
  [steps[index], steps[index + delta]] = [steps[index + delta]!, steps[index]!];
}
</script>
<template>
  <div class="command-editor">
    <label
      >{{ t('Тип действия')
      }}<select
        :value="model.kind"
        :aria-label="t('Тип действия')"
        @change="change(($event.target as HTMLSelectElement).value)"
      >
        <option value="key">{{ t('Клавиша или сочетание') }}</option>
        <option value="media">{{ t('Мультимедиа') }}</option>
        <option value="text">{{ t('Ввести текст') }}</option>
        <option v-if="!step" value="application">{{ t('Открыть приложение') }}</option>
        <option v-if="!step" value="macro">{{ t('Последовательность') }}</option>
        <option v-if="step" value="delay">{{ t('Пауза') }}</option>
      </select></label
    >
    <template v-if="model.kind === 'key'">
      <div class="modifier-pills">
        <label
          v-for="modifier in [
            { bit: 1, name: 'Ctrl' },
            { bit: 2, name: 'Shift' },
            { bit: 4, name: 'Alt' },
            { bit: 8, name: 'Win' },
          ]"
          :key="modifier.bit"
          ><input
            type="checkbox"
            :checked="!!(model.modifiers & modifier.bit)"
            @change="model.modifiers ^= modifier.bit"
          />{{ modifier.name }}</label
        >
      </div>
      <label
        >{{ t('Клавиша')
        }}<select v-model.number="model.key" :aria-label="t('Выходная клавиша')">
          <option
            v-for="key in keyChoices.filter((k) => supportedKey(k.code))"
            :key="key.code"
            :value="key.code"
          >
            {{ key.label }}
          </option>
        </select></label
      >
    </template>
    <label v-else-if="model.kind === 'media'"
      >{{ t('Действие')
      }}<select v-model="model.action">
        <option v-for="a in computerActions" :key="a.id" :value="a.id">{{ t(a.label) }}</option>
      </select></label
    >
    <label v-else-if="model.kind === 'text'"
      >{{ t('Текст')
      }}<textarea
        v-model="model.text"
        rows="4"
        maxlength="2000"
        :aria-label="t('Текст')"
      ></textarea>
    </label>
    <label v-else-if="model.kind === 'application'"
      >{{ t('Приложение')
      }}<select v-model="model.application">
        <option value="word">Microsoft Word</option>
        <option value="notepad">{{ t('Блокнот') }}</option>
        <option value="calculator">{{ t('Калькулятор') }}</option>
      </select></label
    >
    <label v-else-if="model.kind === 'delay'"
      >{{ t('Пауза') }} · {{ model.ms / 1000 }} {{ t('с')
      }}<input v-model.number="model.ms" type="range" min="0" max="5000" step="50"
    /></label>
    <template v-else-if="model.kind === 'macro'">
      <ol class="sequence-steps">
        <li v-for="(_, index) in model.steps" :key="index">
          <div class="step-heading">
            <span>{{ index + 1 }}</span>
            <div>
              <button
                class="text-button"
                :disabled="!index"
                :aria-label="t('Выше')"
                @click="move(index, -1)"
              >
                ↑</button
              ><button
                class="text-button"
                :disabled="index === model.steps.length - 1"
                :aria-label="t('Ниже')"
                @click="move(index, 1)"
              >
                ↓</button
              ><button
                class="text-button"
                :aria-label="t('Удалить шаг')"
                @click="model.steps.splice(index, 1)"
              >
                ×
              </button>
            </div>
          </div>
          <CommandEditor
            :model-value="model.steps[index]!"
            step
            @update:model-value="
              model.kind === 'macro' && (model.steps[index] = $event as ActionStep)
            "
          />
        </li>
      </ol>
      <button
        class="secondary"
        :disabled="model.steps.length >= 128"
        @click="model.steps.push({ kind: 'key', key: 4, modifiers: 0 })"
      >
        + {{ t('Добавить шаг') }}
      </button>
    </template>
  </div>
</template>
