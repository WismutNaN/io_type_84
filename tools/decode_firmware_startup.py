"""Decode pinned IO startup data offline; optionally verify using isolated ARM emulation."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import struct

from inspect_firmware import parse_hex
from analyze_firmware_safety import EXPECTED_SHA256

RAM_BASE = 0x20000000
RAM_LENGTH = 0x7E48


def decompress(data: bytes, size: int) -> tuple[bytes, int]:
    """Transcription of IO's scatter loader at 0x2F4, with explicit bounds."""
    output = bytearray()
    cursor = 0

    def take() -> int:
        nonlocal cursor
        if cursor >= len(data):
            raise ValueError('Truncated compressed input')
        value = data[cursor]
        cursor += 1
        return value

    while len(output) < size:
        command = take()
        literals = command & 7 or take()
        run = command >> 4 or take()
        if literals == 0:
            raise ValueError('Invalid literal count')
        if len(output) + literals - 1 > size:
            raise ValueError('Literal run exceeds destination')
        for _ in range(literals - 1):
            output.append(take())
        if command & 8:
            distance = take()
            if not 0 < distance <= len(output):
                raise ValueError('Invalid back-reference')
            if len(output) + run + 2 > size:
                raise ValueError('Back-reference exceeds destination')
            for _ in range(run + 2):
                output.append(output[-distance])
        else:
            if len(output) + run > size:
                raise ValueError('Zero run exceeds destination')
            output.extend(bytes(run))
    return bytes(output), cursor


def load_image(path: Path) -> bytes:
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != EXPECTED_SHA256:
        raise ValueError('Only the reviewed White 1.17 image is supported')
    memory, _ = parse_hex(raw.decode('ascii'))
    return bytes(memory[i] for i in range(0x16264))


def initialize(image: bytes) -> tuple[bytes, list[dict]]:
    word = lambda at: struct.unpack_from('<I', image, at)[0]
    table_start, table_end = word(0x2EC), word(0x2F0)
    if (table_start, table_end) != (0x15C28, 0x15C48):
        raise ValueError('Unreviewed scatter table')
    ram = bytearray(RAM_LENGTH)
    records = []
    for at in range(table_start, table_end, 16):
        source, destination, length, routine = struct.unpack_from('<4I', image, at)
        start = destination - RAM_BASE
        if start < 0 or start + length > RAM_LENGTH:
            raise ValueError('Scatter destination exceeds reviewed RAM')
        if routine == 0x2F4:
            block, used = decompress(image[source:], length)
        elif routine == 0x115C4 and length % 4 == 0:
            block, used = bytes(length), 0
        else:
            raise ValueError('Unreviewed initialization routine')
        ram[start:start + length] = block
        records.append({'tableAddress': hex(at), 'source': hex(source),
                        'sourceBytesConsumed': used, 'destination': hex(destination),
                        'length': length, 'routine': hex(routine)})
    return bytes(ram), records


def emulate_initializers(image: bytes, records: list[dict]) -> tuple[bytes, int]:
    # Optional research dependency. No peripherals, ROM bootloader or USB are mapped.
    from unicorn import Uc, UC_ARCH_ARM, UC_MODE_THUMB, UC_HOOK_CODE
    from unicorn.arm_const import UC_ARM_REG_R0, UC_ARM_REG_R1, UC_ARM_REG_R2
    from unicorn.arm_const import UC_ARM_REG_SP, UC_ARM_REG_LR, UC_ARM_REG_PC

    cpu = Uc(UC_ARCH_ARM, UC_MODE_THUMB)
    cpu.mem_map(0, 0x20000)
    cpu.mem_write(0, image)
    cpu.mem_map(RAM_BASE, 0x28000)
    # Nonzero backing demonstrates that the actual ZI routine, not map(), clears RAM.
    cpu.mem_write(RAM_BASE, bytes([0xA5]) * 0x28000)
    count = 0

    def guard(uc, address, size, user):
        nonlocal count
        count += 1
        if not (0x2F4 <= address < 0x34A or 0x115C4 <= address < 0x115D2):
            raise ValueError(f'Execution outside initialization allowlist: {address:#x}')

    cpu.hook_add(UC_HOOK_CODE, guard)
    sentinel = 0x1FFF0
    for record in records:
        cpu.reg_write(UC_ARM_REG_R0, int(record['source'], 16))
        cpu.reg_write(UC_ARM_REG_R1, int(record['destination'], 16))
        cpu.reg_write(UC_ARM_REG_R2, record['length'])
        cpu.reg_write(UC_ARM_REG_SP, RAM_BASE + 0x27000)
        cpu.reg_write(UC_ARM_REG_LR, sentinel | 1)
        cpu.emu_start(int(record['routine'], 16) | 1, sentinel, timeout=2_000_000,
                      count=500_000)
        if cpu.reg_read(UC_ARM_REG_PC) != sentinel:
            raise ValueError('Initializer did not return within execution budget')
    return bytes(cpu.mem_read(RAM_BASE, RAM_LENGTH)), count


def report(ram: bytes, records: list[dict]) -> dict:
    word = lambda at: struct.unpack_from('<I', ram, at - RAM_BASE)[0]
    interfaces = []
    for index in range(word(0x20001168)):
        start = 0x116C + index * 24
        length, pointer = struct.unpack_from('<II', ram, start + 12)
        if not (RAM_BASE <= pointer and 0 < length <= 4096 and
                pointer + length <= RAM_BASE + len(ram)):
            raise ValueError('Invalid initialized report descriptor range')
        descriptor = ram[pointer - RAM_BASE:pointer - RAM_BASE + length]
        interfaces.append({'index': index, 'recordAddress': hex(RAM_BASE + start),
                           'reportDescriptorAddress': hex(pointer), 'length': length,
                           'descriptorHex': descriptor.hex(' ')})
    return {
        'schemaVersion': 1, 'method': 'offline-scatter-decode', 'hardwareAccess': False,
        'firmwareSha256': EXPECTED_SHA256, 'ramBase': hex(RAM_BASE), 'ramLength': len(ram),
        'ramSha256': hashlib.sha256(ram).hexdigest(), 'scatterRecords': records,
        'usb': {'interfaceCount': len(interfaces), 'interfaces': interfaces,
                'getReportCallback': hex(word(0x200011EC)),
                'setReportCallback': hex(word(0x200011F0)),
                'lampArrayInterfaceInCallback': 4, 'lampArrayInterfaceAdvertised': False},
        'limits': ['Initial RAM reconstructed from public HEX, not read from device.',
                   'Callback semantics require control-flow analysis, not pointer presence alone.',
                   'No main loop, peripheral driver, ISP or firmware writer executed.'],
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('hex', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--ram-output', type=Path)
    parser.add_argument('--verify-emulation', action='store_true')
    args = parser.parse_args()
    firmware = load_image(args.hex)
    ram, records = initialize(firmware)
    result = report(ram, records)
    if args.verify_emulation:
        actual, instructions = emulate_initializers(firmware, records)
        if actual != ram:
            raise ValueError('Python decoder differs from original ARM routines')
        result['emulation'] = {'engine': 'Unicorn 2.1.4', 'matchedBytes': len(ram),
                               'executedInstructions': instructions,
                               'allowedRoutines': ['0x2f4', '0x115c4'], 'peripheralsMapped': False}
    if args.ram_output:
        args.ram_output.write_bytes(ram)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
    print(json.dumps({'ramSha256': result['ramSha256'], 'interfaces': result['usb']['interfaceCount'],
                      'emulation': result.get('emulation')}))
