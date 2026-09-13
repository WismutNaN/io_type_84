import type { KeyboardSnapshot } from '../src/shared/contracts/generated.ts';
export function fixture(): KeyboardSnapshot {
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
