"""Verify a proposed panel overlay against the original IO LED encoder, offline only."""
from pathlib import Path
import argparse
import hashlib
import json

from decode_firmware_startup import load_image, initialize, RAM_BASE
from analyze_firmware_safety import EXPECTED_SHA256


def verify(path: Path) -> dict:
    from unicorn import Uc, UC_ARCH_ARM, UC_MODE_THUMB, UC_HOOK_CODE, UC_HOOK_MEM_WRITE
    from unicorn.arm_const import UC_ARM_REG_R0, UC_ARM_REG_R1, UC_ARM_REG_R2, UC_ARM_REG_R3
    from unicorn.arm_const import UC_ARM_REG_SP, UC_ARM_REG_LR, UC_ARM_REG_PC
    image = load_image(path)
    ram, _ = initialize(image)
    cpu = Uc(UC_ARCH_ARM, UC_MODE_THUMB)
    cpu.mem_map(0, 0x20000)
    cpu.mem_write(0, image)
    cpu.mem_map(RAM_BASE, 0x28000)
    cpu.mem_write(RAM_BASE, ram)
    allowed = [(0x13028, 0x13052), (0x6C90, 0x6D30)]
    count = 0

    def guard(uc, address, size, user):
        nonlocal count
        count += 1
        if not any(start <= address < end for start, end in allowed):
            raise ValueError(f'Execution outside LED allowlist: {address:#x}')

    cpu.hook_add(UC_HOOK_CODE, guard)
    registers = [UC_ARM_REG_R0, UC_ARM_REG_R1, UC_ARM_REG_R2, UC_ARM_REG_R3]

    def call(address, *args):
        for register, value in zip(registers, args):
            cpu.reg_write(register, value)
        cpu.reg_write(UC_ARM_REG_SP, RAM_BASE + 0x27000)
        cpu.reg_write(UC_ARM_REG_LR, 0x1FFF1)
        cpu.emu_start(address | 1, 0x1FFF0, timeout=2_000_000, count=500_000)
        if cpu.reg_read(UC_ARM_REG_PC) != 0x1FFF0:
            raise ValueError('LED routine did not return within execution budget')

    planes = [0x20005514, 0x20005592, 0x20005610]
    values = [bytes((i * 3 + channel * 11) % 64 for i in range(126)) for channel in range(3)]
    for address, value in zip(planes, values):
        cpu.mem_write(address, value)
    output = 0x20004C34
    # Exact original call sites are pinned; no branch is changed or emitted.
    hooks = [('vendorDispatcher', 0xF94A, bytes.fromhex('fd f7 3b ff'), 0xD7C4),
             ('composeBeforeTransfer', 0xF964, bytes.fromhex('03 f0 60 fb'), 0x13028)]
    for _, at, expected, _ in hooks:
        if image[at:at+4] != expected:
            raise ValueError('Unreviewed hook instruction')
    if image[0xF968:0xF96C] != bytes.fromhex('40 f2 69 42'):
        raise ValueError('Transfer length instruction changed')

    call(0x13028)
    baseline = bytes(cpu.mem_read(output, 1 + 126 * 12))
    panel_map = list(image[0x155BB:0x155CF:2])
    frame = [[(i + 1) * 5, 17, 39] for i in range(10)]
    for index, color in zip(panel_map, frame):
        call(0x6C90, index, *color)
    overlay = bytes(cpu.mem_read(output, len(baseline)))
    changed = [i for i, (a, b) in enumerate(zip(baseline, overlay)) if a != b]
    permitted = {1 + index * 12 + byte for index in panel_map for byte in range(12)}
    if not changed or not set(changed) <= permitted:
        raise ValueError('Overlay changed bytes outside panel slots')
    for address, value in zip(planes, values):
        if bytes(cpu.mem_read(address, 126)) != value:
            raise ValueError('Stock RGB state was modified')
    call(0x13028)  # No host overlay on the next frame: stock restores the output.
    if bytes(cpu.mem_read(output, len(baseline))) != baseline:
        raise ValueError('Stock output was not restored')
    encoder_count = count

    # Execute a discovery packet in the stock dispatcher, with a held macro active.
    # Reject even RAM writes outside the reply/ready flags and this call's stack.
    query = bytes.fromhex('aa 33 04 07 00 01 01 01 49 4f 4c 51') + bytes(52)
    cpu.mem_write(0x2000637C, query)
    cpu.mem_write(0x2000036F, b'\x01')
    cpu.mem_write(0x200000AD, b'\x01')
    allowed.append((0xD7C4, 0xDD40))
    write_ranges = [(0x2000633C, 0x2000637C), (0x2000036E, 0x20000370),
                    (0x20026F00, 0x20027000)]
    written = set()

    def guard_write(uc, access, address, size, value, user):
        if not any(start <= address and address + size <= end for start, end in write_ranges):
            raise ValueError(f'Discovery wrote outside response/stack: {address:#x}')
        written.update(range(address, address + size))

    cpu.hook_add(UC_HOOK_MEM_WRITE, guard_write)
    call(0xD7C4)
    stock_reply = bytes(cpu.mem_read(0x2000633C, 64))
    if stock_reply != b'\x55' + query[1:]:
        raise ValueError('Stock discovery was not an echo')
    if bytes(cpu.mem_read(0x200000AD, 1)) != b'\x01':
        raise ValueError('Stock discovery interrupted the held macro')
    if bytes(cpu.mem_read(0x2000036E, 2)) != b'\x01\x00':
        raise ValueError('Stock dispatcher did not consume RX and mark TX ready')
    return {
        'schemaVersion': 1, 'method': 'isolated-original-ARM-LED-emulation',
        'firmwareSha256': EXPECTED_SHA256,
        'hardwareAccess': False, 'peripheralsMapped': False, 'patchedImageGenerated': False,
        'engine': 'Unicorn 2.1.4', 'executedInstructions': count,
        'allowedRoutines': [hex(start) for start, _ in allowed],
        'hookCandidates': [{'role': name, 'callSite': hex(at), 'originalBytes': code.hex(' '),
                            'originalTarget': hex(target), 'replacementTarget': None}
                           for name, at, code, target in hooks],
        'wire': {'encodedBytesPerLed': 12, 'prefixBytes': 1, 'transferBytes': 0x469,
                 'ledSlotsCoveredByTransfer': (0x469 - 1) // 12,
                 'stockEncodedSlots': 126, 'panelRenderMap': panel_map,
                 'panelOutputByteRange': [1 + 84 * 12, 1 + 94 * 12]},
        'checks': {'changedBytes': len(changed), 'changesOnlyInPanel': True,
                   'stockRgbPlanesPreserved': True, 'releaseRestoresNextEncodedFrame': True},
        'stockDiscovery': {
            'requestHex': query.hex(' '), 'responseHex': stock_reply.hex(' '),
            'executedInstructions': count - encoder_count,
            'echoOnly': True, 'heldMacroFlagPreserved': True,
            'writesOnlyResponseReadyFlagsAndStack': True,
            'distinctWrittenBytes': len(written), 'hardwareProbeSent': False,
        },
        'baselineSha256': hashlib.sha256(baseline).hexdigest(),
        'overlaySha256': hashlib.sha256(overlay).hexdigest(),
        'limits': ['Tests selected encoder routines, not the complete device or timing.',
                   'SPI transfer itself was not executed; length is a static call-site observation.',
                   'Physical direction, channel order, and hardware recovery remain unverified.',
                   'No code-cave address, ABI trampoline or MCU RAM allocation approved.'],
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('hex', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = verify(args.hex)
    args.output.write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(result['checks']))
