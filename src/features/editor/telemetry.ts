import type { MonitorFrame } from '../../shared/contracts/generated.ts';

export const SAMPLE_TTL_MS = 600;
// Время IPC-снимка не является часами: возраст продолжает расти при зависшем запросе.
export function ageFrame(frame: MonitorFrame, elapsed: number): MonitorFrame {
  const age = Math.max(0, elapsed);
  return {
    ...frame,
    travel: frame.travel.map((k) => ({ ...k, ageMs: k.ageMs + age })),
    colorAgeMs: frame.colorAgeMs === null ? null : frame.colorAgeMs + age,
  };
}
export function sampleDepth(frame: MonitorFrame, slot: number): number | null {
  const sample = frame.travel.find((k) => k.slot === slot);
  return frame.active && sample && sample.ageMs < SAMPLE_TTL_MS ? sample.travelUm : null;
}
