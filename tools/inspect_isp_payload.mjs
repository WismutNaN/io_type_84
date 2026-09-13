/** Offline inspection of the pinned IO Vision HEX loader. No browser, HID or network imports.
 * NEVER call start_isp, wasm_initialize or __wbindgen_start here.
 * Usage: node tools/inspect_isp_payload.mjs vendor.wasm firmware.hex output.json
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';

const [wasmPath, hexPath, outputPath] = process.argv.slice(2);
if (!wasmPath || !hexPath || !outputPath) throw new Error('Expected wasm, hex, output paths');
const wasm = readFileSync(wasmPath), hex = readFileSync(hexPath);
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
if (sha256(wasm) !== '8816f7c9a112265943f0137d1d54b4c9fc13d87850803918d5e53a9569aacba6')
  throw new Error('Unreviewed WASM image');
if (sha256(hex) !== '8f5ef507771c6795258eb7521cfc1b46269a5b301548767ae87a3f1a83a50d45')
  throw new Error('Unreviewed HEX image');

const module = new WebAssembly.Module(wasm);
// Every host import fails closed, even logging/timers. The loader is pure WASM.
// Instantiation of this pinned module has no start section.
const imports = {};
const blockedImports = [];
for (const item of WebAssembly.Module.imports(module)) {
  if (item.kind !== 'function') throw new Error(`Unexpected import kind: ${item.kind}`);
  imports[item.module] ??= {};
  imports[item.module][item.name] = () => {
    blockedImports.push(item.name);
    throw new Error(`Host access prohibited: ${item.name}`);
  };
}
const instance = new WebAssembly.Instance(module, imports);
const exports = instance.exports;
const allowed = new Set(['__wbindgen_malloc', 'snx_isp_load_hex_buffer']);
function call(name, ...args) {
  if (!allowed.has(name)) throw new Error(`Export prohibited: ${name}`);
  return exports[name](...args);
}
const input = call('__wbindgen_malloc', hex.length, 1);
new Uint8Array(exports.memory.buffer, input, hex.length).set(hex);
const result = call('snx_isp_load_hex_buffer', input, hex.length);
if (result[2] !== 0) throw new Error(`Vendor HEX loader failed: ${result}`);
const memory = new DataView(exports.memory.buffer);
const u32 = (at) => memory.getUint32(at, true);
const profile = u32(1059240), payload = u32(1059228), capacity = u32(1059232);
const words = Array.from({ length: 18 }, (_, i) => u32(profile + i * 4));
const programLength = words[2]; // Confirmed in func185 checksum loop; profile func66.
if (programLength !== 0x7e000 || capacity !== 0x100000) throw new Error('Layout changed');
const bytes = new Uint8Array(exports.memory.buffer, payload, programLength);
const calibration = (offset) => Array.from({ length: 128 }, (_, i) =>
  memory.getUint16(payload + offset + i * 2, true));
const report = {
  schemaVersion: 1,
  mode: 'offline-vendor-loader-with-all-host-imports-denied',
  wasmSha256: sha256(wasm), hexSha256: sha256(hex),
  calledExports: [...allowed], blockedImports, hardwareAccess: false,
  vendorLoaderReturn: result, profileWords: words.map(v => `0x${v.toString(16)}`),
  programLength, bufferCapacity: capacity, payloadSha256: sha256(bytes),
  absentHexAreaSample: Array.from(bytes.subarray(0x20000, 0x20010)),
  footer: Array.from(bytes.subarray(0x7dffc, 0x7e000)),
  calibrationMax: calibration(0x8400), calibrationMin: calibration(0x8500),
  limitation: 'No ISP state machine or device executed; this is not a device backup.'
};
writeFileSync(outputPath, JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({ programLength, payloadSha256: report.payloadSha256,
  vendorLoaderReturn: result, hostImportCalls: blockedImports.length }));
