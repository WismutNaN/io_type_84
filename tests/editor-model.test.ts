import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  clone,
  mixColor,
  profileEdits,
  projectEdits,
  readLocalProfile,
} from '../src/features/editor/model.ts';
import type { KeyboardSnapshot } from '../src/shared/contracts/generated.ts';
import { importVisionProfile } from '../src/features/editor/io-vision-profile.ts';
import { physicalKeyCodes } from '../src/shared/keyboard-view/keycodes.ts';

function fixture(): KeyboardSnapshot {
  return {
    revision: 'test',
    identity: {
      name: 'IO Type 84 Magnetic White',
      vendorId: 3141,
      productId: 32982,
      firmware: '1.17',
      frameVersion: 0,
      rtPrecision: 0,
    },
    keys: Array.from({ length: 128 }, (_, slot) => ({
      slot,
      ledId: slot,
      base: { page: 0, parameters: [0, 0, 0] },
      function: { page: 0, parameters: [0, 0, 0] },
      color: { r: 0, g: 0, b: 0 },
      actuation: {
        triggerUm: 1200,
        pressUm: 0,
        releaseUm: 0,
        rapidTrigger: false,
        wholeTravel: false,
        rampage: false,
        axisType: 0,
      },
    })),
    lighting: {
      mode: 11,
      color: { r: 255, g: 255, b: 255 },
      secondaryColor: { r: 0, g: 0, b: 0 },
      colorMode: 1,
      brightness: 5,
      speed: 3,
      direction: 0,
    },
    performance: { reportRate: 6, topDeadZoneUm: 0, bottomDeadZoneUm: 0, keyDelay: 0 },
    dks: Array.from({ length: 64 }, (_, index) => ({
      index,
      thresholds: [0, 0, 0, 0],
      actions: [0, 0, 0, 0],
      states: [0, 0, 0, 0],
    })),
    macros: [],
    macroBytesUsed: 400,
    macroWriteLimit: 512,
  };
}
const profile = () => ({
  schemaVersion: 1,
  name: 'Test',
  savedAt: new Date().toISOString(),
  snapshot: fixture(),
});

test('import rejects nested corruption without passing malformed data to the editor', () => {
  assert.equal(readLocalProfile(JSON.stringify(profile())).snapshot.keys.length, 128);
  for (const corrupt of [
    (p: any) => (p.snapshot.keys[49].base.parameters = '004'),
    (p: any) => (p.snapshot.keys[49].actuation.triggerUm = '1200'),
    (p: any) => (p.snapshot.lighting.color = null),
    (p: any) => (p.snapshot.dks[0].states = [1, 2]),
    (p: any) => (p.snapshot.dks[0].index = 63),
    (p: any) => (p.snapshot.macros = [{ id: 0, steps: [null] }]),
    (p: any) =>
      (p.snapshot.macros = [
        { id: 0, steps: [] },
        { id: 0, steps: [] },
      ]),
  ]) {
    const p = profile();
    corrupt(p);
    assert.throws(() => readLocalProfile(JSON.stringify(p)));
  }
});

test('pair import preserves the selected D/A order even though slots are A/D', () => {
  const source = fixture(),
    target = clone(source);
  target.keys[49]!.base = { page: 11, parameters: [1, 7, 4] };
  target.keys[51]!.base = clone(target.keys[49]!.base);
  const edits = profileEdits(source, target);
  assert.deepEqual(edits, [
    {
      kind: 'binding',
      slots: [51, 49],
      functionLayer: false,
      binding: { page: 11, parameters: [1, 7, 4] },
    },
  ]);
  assert.deepEqual(projectEdits(source, edits).keys, target.keys);
});

test('replacing a pair cannot leave an active orphan; source remains immutable', () => {
  const source = fixture();
  source.keys[49]!.base = { page: 12, parameters: [0, 4, 7] };
  source.keys[51]!.base = clone(source.keys[49]!.base);
  const next = projectEdits(source, [
    {
      kind: 'binding',
      slots: [49],
      functionLayer: false,
      binding: { page: 2, parameters: [0, 5, 0] },
    },
  ]);
  assert.equal(next.keys[51]!.base.page, 0);
  assert.equal(source.keys[51]!.base.page, 12);
  const target = clone(source);
  target.keys[49]!.base = { page: 2, parameters: [0, 6, 0] };
  target.keys[51]!.base = { page: 2, parameters: [0, 7, 0] };
  assert.deepEqual(projectEdits(source, profileEdits(source, target)).keys, target.keys);
});

test('gradient endpoints and selected-key edits do not affect other keys', () => {
  const a = { r: 220, g: 20, b: 10 },
    b = { r: 10, g: 120, b: 240 };
  assert.deepEqual(mixColor(a, b, 0), a);
  assert.deepEqual(mixColor(a, b, 1), b);
  const source = fixture(),
    next = projectEdits(source, [{ kind: 'color', slots: [49], color: mixColor(a, b, 0.5) }]);
  assert.deepEqual(next.keys[50], source.keys[50]);
  assert.deepEqual(next.keys[49]!.color, { r: 115, g: 70, b: 125 });
});

test('IO Vision null thresholds and absent bindings do not reset the device', () => {
  const source = fixture();
  source.keys[49]!.actuation.triggerUm = 2230;
  source.keys[49]!.base = { page: 2, parameters: [0, 5, 0] };
  const keys = Object.entries(physicalKeyCodes).map(([slot, keyCode]) => ({
    value: Number(slot),
    keyCode,
  }));
  const value = {
    deviceId: '3141:32982:IO Type 84 Magnetic White',
    profile: {
      keyList: Array.from({ length: 6 }, (_, i) => keys.slice(i * 14, i * 14 + 14)),
      magneticAxisRT: Array.from({ length: 128 }, () => ({
        triggerKeyStroke: 0,
        pressRT: 0,
        releaseRT: 0,
        isWholeFast: false,
        isRampageMode: false,
      })),
    },
  };
  const imported = importVisionProfile(JSON.stringify(value), source);
  assert.equal(imported.edits.length, 0);
  assert.ok(imported.warnings.some((s) => s.includes('84 нулевых')));
  assert.deepEqual(projectEdits(source, imported.edits), source);
  value.profile.keyList[0]![0]!.value = 127;
  assert.throws(() => importVisionProfile(JSON.stringify(value), source));
});
