import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ageFrame, sampleDepth } from '../src/features/editor/telemetry.ts';
import type { MonitorFrame } from '../src/shared/contracts/generated.ts';
test('a frozen IPC frame expires without inventing a zero-depth release', () => {
  const frame: MonitorFrame = {
    active: true,
    travel: [{ slot: 49, travelUm: 2100, maxTravelUm: 3200, ageMs: 100, adc: 0 }],
    history: [],
    colors: [],
    colorAgeMs: null,
    packets: 1,
    message: null,
  };
  assert.equal(sampleDepth(ageFrame(frame, 200), 49), 2100);
  assert.equal(sampleDepth(ageFrame(frame, 500), 49), null);
  assert.equal(frame.travel[0]?.ageMs, 100);
  assert.equal(sampleDepth({ ...frame, active: false }, 49), null);
});
