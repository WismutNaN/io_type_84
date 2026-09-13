"""Pinned, offline White 1.17 evidence. No device transport, executable output or patcher."""
from __future__ import annotations
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import struct
import xml.etree.ElementTree as ET

from inspect_firmware import parse_hex

EXPECTED_SHA256 = '8f5ef507771c6795258eb7521cfc1b46269a5b301548767ae87a3f1a83a50d45'
USER_ROM_BYTES = 0x7E000

# Analyst-assigned names; they are not symbols recovered from the vendor source.
SYMBOLS = {
    0x1D8: ('enter_boot_rom', 'boot'),
    0x204: ('reset_entry', 'boot'),
    0x3A4: ('usb_hs_interrupt', 'usb'),
    0xBF0: ('emit_analog_fb', 'analog'),
    0x1464: ('fmc_page_erase_request', 'flash'),
    0x15C8: ('fmc_mass_erase_request', 'flash'),
    0x172C: ('fmc_page_program_request', 'flash'),
    0x18D8: ('fmc_wait_status', 'flash'),
    0x1930: ('finish_calibration_and_persist', 'calibration'),
    0x1F30: ('flash_erase_dispatch', 'flash'),
    0x1FF8: ('flash_program_dispatch', 'flash'),
    0x2188: ('watchdog_stop', 'runtime'),
    0x221C: ('watchdog_reload', 'runtime'),
    0x2274: ('initialize_flash_remap_offset', 'flash'),
    0x3608: ('update_calibration_min_ram', 'calibration'),
    0x3630: ('flash_patch_pages', 'flash'),
    0x372C: ('flash_range_check_stub', 'flash'),
    0x3738: ('dispatch_firmware_action', 'actions'),
    0x3AA8: ('process_magnetic_sample', 'analog'),
    0x41CC: ('initialize_runtime_from_flash', 'boot'),
    0x4F60: ('service_boot_and_deferred_writes', 'boot'),
    0x51AE: ('cycle_panel_mode', 'lighting'),
    0x54C4: ('persist_panel_and_options', 'lighting'),
    0x6C14: ('filter_adc_float', 'analog'),
    0xD7C4: ('dispatch_vendor_aa55', 'usb'),
    0xE540: ('render_panel', 'lighting'),
    0xEB60: ('render_panel_depth', 'lighting'),
    0xEEA0: ('render_panel_depth_overlay', 'lighting'),
    0xF4D8: ('matches_isp_magic', 'boot'),
    0xF8E0: ('service_periodic_work', 'runtime'),
    0x13028: ('compose_lighting_output', 'lighting'),
    0x1376C: ('usb_report_callback', 'usb'),
}

RAM_LABELS = {
    0x2000006C: 'runtime_flags_bit4_boot_request',
    0x2000036C: 'calibration_mode', 0x2000036D: 'simulation_mode',
    0x2000353A: 'calibration_max_128_u16', 0x2000363A: 'calibration_min_128_u16',
    0x20003BBA: 'filtered_adc_by_scan_slot',
    0x20005979: 'flash_page_scratch',
}

def canonical_payload(memory: dict[int, int]) -> bytes:
    """Independent model of the observed vendor loader's zero-filled USER ROM buffer.

    This function produces analysis bytes in RAM, not a file suitable for flashing.
    """
    if any(a < 0 or a >= USER_ROM_BYTES for a in memory):
        raise ValueError('HEX exceeds the reviewed USER ROM range')
    result = bytearray(USER_ROM_BYTES)
    for address, value in memory.items():
        result[address] = value
    return bytes(result)

def analyze(path: Path, dfp: Path | None = None) -> dict:
    raw = path.read_bytes()
    sha = hashlib.sha256(raw).hexdigest()
    if sha != EXPECTED_SHA256:
        raise ValueError('Unreviewed firmware image; address evidence would be invalid')
    memory, _ = parse_hex(raw.decode('ascii'))
    payload = canonical_payload(memory)
    def word(a):
        return int.from_bytes(bytes(memory[a+i] for i in range(4)), 'little')
    def table(a):
        return [int.from_bytes(payload[i:i+2], 'little') for i in range(a, a+256, 2)]
    result = {
        'schemaVersion': 1, 'method': 'offline-static-analysis', 'hardwareAccess': False,
        'firmwareSha256': sha, 'mappedHexBytes': len(memory),
        'canonicalPayload': {
            'length': len(payload), 'holeFill': 0,
            'sha256': hashlib.sha256(payload).hexdigest(),
            'sum16LittleEndian': sum(struct.unpack('<%dH' % (len(payload)//2), payload)) & 0xffff,
            'notADeviceBackup': True,
        },
        'ispEntry': {
            'magicLiteralAt': '0x0000f4f4', 'magic': payload[0xf4f4:0xf4fc].hex(' '),
            'callback': '0x0001376c', 'flagAddress': hex(word(0x138cc)), 'flagMask': 16,
            'consumer': '0x00004f60', 'jumpRoutine': '0x000001d8',
            'bootRomThumbTarget': hex(word(0x200)),
            'userModeFlagAddress': '0x0007dffc', 'userModeFlagValue': hex(word(0x7dffc)),
        },
        'calibration': {
            'countPerTable': 128, 'maxFlash': '0x8400', 'minFlash': '0x8500',
            'maxRam': hex(word(0x1980)), 'minRam': hex(word(0x197c)),
            'maxValues': dict(Counter(table(0x8400))), 'minValues': dict(Counter(table(0x8500))),
            'minStatusMask': '0x8000', 'minAdcMask': '0x7fff',
            'stopCommand': '0x65', 'stopCallsPersistAt': '0xdb1a',
            'persistFunction': '0x1930', 'flashWriter': '0x3630',
            'pageBytesObservedInAppWriter': 512, 'sameErasePage': True,
            'uniqueUnitValuesAcquired': False,
        },
        'symbols': [{'address': f'0x{a:08x}', 'name': n, 'subsystem': s,
                     'basis': 'analyst-assigned; verify control flow and types'}
                    for a, (n, s) in SYMBOLS.items()],
    }
    if dfp is not None:
        startup = dfp / 'Device/Source/ARM/startup_SN34F280.s'
        header = dfp / 'Device/Include/SN34F280.h'
        svd = dfp / 'SVD/SN34F280.svd'
        source = startup.read_text(encoding='utf-8-sig')
        vectors = []
        for symbol in re.findall(r'^.*?\bDCD\s+(\w+)', source, re.M)[:112]:
            at = len(vectors) * 4
            vectors.append({'offset': hex(at), 'sdkName': symbol, 'thumbTarget': hex(word(at))})
        peripherals = []
        # Occurrence of a number is a candidate reference, not proof of peripheral use.
        for p in ET.parse(svd).getroot().findall('./peripherals/peripheral'):
            base = int(p.findtext('baseAddress'), 0)
            refs = [hex(a) for a in range(0, 0x16260, 4) if word(a) == base]
            if refs:
                peripherals.append({'name': p.findtext('name'), 'base': hex(base),
                                    'literalCandidates': refs})
        result['sdk'] = {
            'package': 'SONiX.SN34F2_DFP.2.0.4',
            'files': {str(p.relative_to(dfp)): hashlib.sha256(p.read_bytes()).hexdigest()
                      for p in (startup, header, svd)},
            'vectorSlots': vectors, 'peripheralLiteralCandidates': peripherals,
            'footerDocumentedAsUserModeFlag': 'ISP_MODE_FLAG = 0xAAAA5555 for USER MODE' in source,
        }
    return result

def render_atlas(report: dict, source: Path, output: Path, hex_path: Path):
    """Readable local atlas. The vendor-derived pseudocode stays outside Git."""
    memory, _ = parse_hex(hex_path.read_text(encoding='ascii'))
    symbols = {int(s['address'], 16): s for s in report['symbols']}
    peripheral = {int(p['base'], 16): p['name']
                  for p in report.get('sdk', {}).get('peripheralLiteralCandidates', [])}
    index = json.loads((source / 'functions.json').read_text(encoding='utf-8'))
    output.mkdir(parents=True, exist_ok=True)
    lines = ['# White 1.17 — локальный атлас', '',
             '**Псевдокод, не собираемая прошивка.** Имена назначены исследователем.', '',
             '| Адрес | Подсистема | Псевдокод |', '|---|---|---|']
    count = 0
    for entry in index:
        at = int(entry['address'], 16)
        symbol = symbols.get(at, {'name': entry['name'], 'subsystem': 'unclassified'})
        original = source / 'functions' / (entry['address'] + '.c')
        if not original.exists() or not entry.get('decompiled'):
            continue
        code = original.read_text(encoding='utf-8')
        literals = []
        for a in sorted(set(int(v, 16) for v in re.findall(r'DAT_([0-9a-fA-F]{8})', code))):
            if all(a+i in memory for i in range(4)):
                value = int.from_bytes(bytes(memory[a+i] for i in range(4)), 'little')
                name = RAM_LABELS.get(value, peripheral.get(value, ''))
                literals.append(f' * word at 0x{a:08x} = 0x{value:08x} {name}')
        for s_at, s in symbols.items():
            code = code.replace(f'FUN_{s_at:08x}', s['name'])
        directory = output / symbol['subsystem']
        directory.mkdir(exist_ok=True)
        file = directory / (entry['address'] + '_' + symbol['name'] + '.c')
        prefix = '/* ANALYSIS ONLY. Not original source; inferred types may be wrong.\n'
        prefix += f' * Entry: 0x{at:08x}; subsystem: {symbol["subsystem"]}\n'
        prefix += ' * Literal words below are observations, not type declarations.\n'
        prefix += '\n'.join(literals) + '\n */\n'
        file.write_text(prefix + code, encoding='utf-8')
        lines.append(f'| 0x{at:08x} | {symbol["subsystem"]} | [{symbol["name"]}]({file.relative_to(output).as_posix()}) |')
        count += 1
    (output/'README.md').write_text('\n'.join(lines)+'\n', encoding='utf-8')
    return count

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('hex', type=Path)
    parser.add_argument('--dfp', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--decompiled', type=Path)
    parser.add_argument('--atlas', type=Path)
    args = parser.parse_args()
    result = analyze(args.hex, args.dfp)
    if args.atlas and args.decompiled:
        result['atlasFunctionCandidates'] = render_atlas(result, args.decompiled, args.atlas, args.hex)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
    print(json.dumps({'evidence': str(args.output), 'payload': result['canonicalPayload'],
                      'atlasFunctionCandidates': result.get('atlasFunctionCandidates')}))
