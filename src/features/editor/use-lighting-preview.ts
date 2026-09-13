import { computed, onMounted, onScopeDispose, ref, type Ref } from 'vue';
import type { KeyboardSnapshot, LightingSettings } from '../../shared/contracts/generated';
import { keyboardKeys } from '../../shared/keyboard-view/layout';
import { effectColor, type LightPulse } from './lighting-preview';

export function useLightingPreview(
  light: Ref<LightingSettings>,
  snapshot: Ref<KeyboardSnapshot | null>,
  enabled: Ref<boolean>,
) {
  const playing = ref(true),
    seconds = ref(0),
    pulses = ref<LightPulse[]>([]);
  let frame = 0,
    previous = 0;
  const motion = matchMedia('(prefers-reduced-motion: reduce)');
  playing.value = !motion.matches;
  const changeMotion = () => {
    playing.value = !motion.matches;
  };
  motion.addEventListener('change', changeMotion);
  const tick = (time: number) => {
    if (time - previous >= 33) {
      if (enabled.value && playing.value && !document.hidden)
        seconds.value += Math.min((time - previous) / 1000, 0.1);
      previous = time;
    }
    frame = requestAnimationFrame(tick);
  };
  onMounted(() => {
    frame = requestAnimationFrame(tick);
  });
  onScopeDispose(() => {
    cancelAnimationFrame(frame);
    motion.removeEventListener('change', changeMotion);
  });
  function pulse(slot: number) {
    const key = keyboardKeys.find((k) => k.slot === slot);
    if (!key) return;
    pulses.value = [
      ...pulses.value.filter((p) => seconds.value - p.at < 2).slice(-15),
      {
        slot,
        at: seconds.value,
        x: (key.x + key.width / 2) / 1044,
        y: (key.y + key.height / 2) / 356,
      },
    ];
  }
  const colors = computed(() =>
    enabled.value
      ? Object.fromEntries(
          keyboardKeys.map((key) => [
            key.slot,
            effectColor(
              light.value,
              key,
              seconds.value,
              snapshot.value?.keys[key.slot]?.color ?? { r: 255, g: 255, b: 255 },
              pulses.value,
            ),
          ]),
        )
      : undefined,
  );
  return { colors, playing, pulse };
}
