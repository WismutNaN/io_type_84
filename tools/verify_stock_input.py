"""Execute stock White 1.17 input routines offline; never opens a device or emits firmware."""
from pathlib import Path
import argparse
import json
import struct
from itertools import product

from decode_firmware_startup import load_image, initialize, RAM_BASE
from analyze_firmware_safety import EXPECTED_SHA256


class StockInput:
    def __init__(self, image):
        from unicorn import Uc, UC_ARCH_ARM, UC_MODE_THUMB, UC_HOOK_CODE, UC_HOOK_MEM_WRITE
        from unicorn.arm_const import UC_ARM_REG_PC, UC_ARM_REG_SP, UC_ARM_REG_LR
        self.pc, self.sp, self.lr = UC_ARM_REG_PC, UC_ARM_REG_SP, UC_ARM_REG_LR
        self.cpu = Uc(UC_ARCH_ARM, UC_MODE_THUMB)
        self.cpu.mem_map(0, 0x20000)
        self.cpu.mem_write(0, image)
        self.cpu.mem_map(RAM_BASE, 0x28000)
        ram, _ = initialize(image)
        self.cpu.mem_write(RAM_BASE, ram)
        self.image = image
        self.events = []
        self.visited = set()
        self.instructions = 0
        self.cpu.hook_add(UC_HOOK_CODE, self.trace)
        self.cpu.hook_add(UC_HOOK_MEM_WRITE, self.guard_write)

    def guard_write(self, cpu, access, address, size, value, user):
        if not RAM_BASE <= address < address + size <= RAM_BASE + 0x28000:
            raise ValueError(f'Non-RAM write at {address:#x}')

    def trace(self, cpu, address, size, user):
        self.instructions += 1
        self.visited.add(address)
        # Stop before any flash/boot primitive even though no peripherals are mapped.
        if address in (0x1464, 0x15C8, 0x172C, 0x1930, 0x3630, 0x1D8, 0x54C4):
            raise ValueError(f'Forbidden routine {address:#x}')
        if address == 0x112A8:
            self.events.append({'code': self.read8(0x20000089),
                                'pressed': self.read8(0x2000008D) == 1})

    def read8(self, address):
        return self.cpu.mem_read(address, 1)[0]

    def write8(self, address, value):
        self.cpu.mem_write(address, bytes([value]))

    def call(self, address):
        self.cpu.reg_write(self.sp, RAM_BASE + 0x27000)
        self.cpu.reg_write(self.lr, 0x1FFF1)
        self.cpu.emu_start(address | 1, 0x1FFF0, timeout=1_000_000, count=100_000)
        if self.cpu.reg_read(self.pc) != 0x1FFF0:
            raise ValueError(f'Routine {address:#x} did not return')

    def configure(self, slot, binding):
        indices = [i for i in range(128) if self.image[0x13A90 + i] == slot]
        if len(indices) != 1:
            raise ValueError('Ambiguous physical scan mapping')
        self.scan = indices[0]
        self.slot = slot
        # Configuration fixtures inside the isolated emulator, not a patch or SET.
        self.cpu.mem_write(0x9600 + slot * 4, bytes(binding))
        self.cpu.mem_write(0x2000253A + slot * 4, bytes(binding))
        self.write8(0x20000088, self.scan)
        self.write8(0x20000087, self.scan >> 3)
        self.write8(0x20000086, self.scan & 7)

    def digital(self, pressed):
        self.write8(0x20000089, self.image[0x13B38 + self.scan])
        self.write8(0x2000008D, int(pressed))
        self.call(0x4FC8)

    def depth(self, units):
        self.cpu.mem_write(0x2000343A + self.scan * 2, struct.pack('<H', units))
        self.call(0x43FC)

    def drain_taps(self):
        # Model completion of USB report consumption; USB timing is not emulated.
        for _ in range(9):
            self.write8(0x2000009E, 0)
            self.write8(0x2000009F, 0)
            if self.read8(0x20000102) == self.read8(0x20000103):
                return
            self.call(0x1230)
        raise ValueError('Tap queue did not drain')

    def drain_mt(self):
        self.write8(0x2000009E, 0)  # model the USB consumer, as for DKS
        self.call(0x4394)


def dks_cycle(image, states, depths, actions=(75, 0xB9, 0, 0), modifiers=(0, 0, 0, 0)):
    m = StockInput(image)
    m.configure(105, [8, 0, 0, 0])
    pairs = [byte for pair in zip(modifiers, actions) for byte in pair]
    m.cpu.mem_write(0xB200, bytes([16, 30, 29, 16, *pairs, *states]))
    trace = []
    for depth in depths:
        start = len(m.events)
        m.depth(depth)
        modifier_before_drain = m.read8(0x200000E6)
        consumer_before_drain = bytes(m.cpu.mem_read(0x200000BC, 3)).hex(' ')
        m.drain_taps()
        trace.append({'depthUnits10um': depth, 'emissionRequests': m.events[start:],
                      'modifierMaskBeforeDrain': modifier_before_drain,
                      'consumerBeforeDrain': consumer_before_drain})
    return {'states': list(states), 'trace': trace, 'instructions': m.instructions}


def verify(path):
    image = load_image(path)
    result = {'schemaVersion': 1, 'firmwareSha256': EXPECTED_SHA256, 'hardwareAccess': False,
              'method': 'isolated-original-ARM-input-emulation', 'bindings': [], 'dks': [],
              'limits': ['Configuration fixtures in emulated ROM/RAM; no firmware file generated.',
                         'Emission requests and report buffers, not captured USB packets.',
                         'ADC, USB transfer, real timing and power-cycle persistence not emulated.',
                         'USB completion is modeled by clearing dirty flags before stock release queues.',
                         'Only the specified input paths and fixtures are covered.']}
    for name, binding in [('default', [0, 0, 0, 0]), ('keyboardZero', [2, 0, 0, 0]),
                          ('emptyExtraFunction', [5, 0, 0, 0]), ('f13', [2, 0, 0x68, 0]),
                          ('volumeAlias', [2, 0, 0xB9, 0])]:
        m = StockInput(image)
        m.configure(105, binding)
        m.digital(True)
        consumer_down = bytes(m.cpu.mem_read(0x200000BC, 3)).hex(' ')
        m.digital(False)
        result['bindings'].append({'name': name, 'bytes': binding, 'events': m.events,
                                   'consumerDown': consumer_down,
                                   'consumerUp': bytes(m.cpu.mem_read(0x200000BC, 3)).hex(' ')})
    cycles = [('shallow', [0, 170, 170, 0, 0]), ('deep', [0, 170, 310, 310, 250, 0, 0])]
    for states in ([1, 2, 0, 0], [0, 2, 0, 1], [0x10, 2, 0, 0]):
        for name, depths in cycles:
            result['dks'].append({'cycle': name, **dks_cycle(image, states, depths)})

    def downs(cycle):
        return [e['code'] for t in cycle['trace'] for e in t['emissionRequests'] if e['pressed']]

    matches = []
    for states in product((0, 1, 16), repeat=4):
        shallow, deep = [dks_cycle(image, states, depths, (75, 0, 0, 0)) for _, depths in cycles]
        if 75 in downs(shallow) and 75 not in downs(deep):
            matches.append(list(states))
    result['dksExclusiveSearch'] = {'configurations': 81, 'phaseChoices': ['off', 'tap', 'hold'],
        'thresholds': [16, 30, 29, 16], 'shallowOnlyPgUpMatches': matches,
        'scope': 'One output action, all 3^4 valid phase states, two specified cycles; not all thresholds.'}
    result['dksModifier'] = dks_cycle(image, [1, 0, 0, 0], cycles[0][1], modifiers=(2, 0, 0, 0))

    result['modTap'] = []
    for ticks in (10, 200, 250):
        m = StockInput(image)
        m.configure(105, [9, 0xB9, 75, 20])
        m.digital(True)
        for _ in range(ticks):
            m.call(0x4348)
        m.digital(False)
        m.drain_mt()
        result['modTap'].append({'timerTicksBeforeRelease': ticks, 'emissionRequests': m.events})

    result['macroKinds'] = []
    for kind in (1, 3):
        m = StockInput(image)
        m.configure(105, [6, 0, 1, 1])
        m.cpu.mem_write(0x9C00, struct.pack('<I', 400))
        m.cpu.mem_write(0x9C00 + 400, bytes([4, 0, 0, 0, 1, 0, 4, kind * 16 + 128, 1, 0, 4, kind * 16]))
        m.digital(True)
        buttons = []
        for _ in range(5):
            m.call(0x3310)
            buttons.append(m.read8(0x200000E1))
        result['macroKinds'].append({'wireKind': kind, 'emissionRequests': m.events, 'mouseButtons': buttons})

    result['simulation'] = []
    m = StockInput(image)
    m.configure(105, [0, 0, 0, 0])
    for command in (0x66, 0x67):
        packet = bytes([0xAA, command, 0, 0, 0, 1, 1, 1]) + bytes(56)
        m.cpu.mem_write(0x2000637C, packet)
        m.write8(0x2000036F, 1)
        m.call(0xD7C4)
        start = len(m.events)
        m.digital(True)
        m.digital(False)
        result['simulation'].append({'command': command, 'simulationFlag': m.read8(0x2000036D),
                                     'emissionRequests': m.events[start:]})

    result['osMode'] = []
    for mode in (0, 1):
        for slot, code in ((81, 227), (82, 226)):
            m = StockInput(image)
            m.configure(slot, [2, 0, code, 0])
            m.write8(0x200000A8, mode)
            m.digital(True)
            m.digital(False)
            result['osMode'].append({'mode': mode, 'slot': slot, 'bindingCode': code,
                                     'emissionRequests': m.events})
    result['mediaAliases'] = []
    for code in (0xB0, 0xB1, 0xB3, 0xB5, 0xB9, 0xBA):
        m = StockInput(image)
        m.configure(105, [2, 0, code, 0])
        m.digital(True)
        down = bytes(m.cpu.mem_read(0x200000BC, 3))
        m.digital(False)
        assert bytes(m.cpu.mem_read(0x200000BC, 3)) == bytes([3, 0, 0])
        assert down[0] == 3
        result['mediaAliases'].append({'internalCode': code, 'consumerUsage': int.from_bytes(down[1:], 'little')})

    assert result['bindings'][0]['events'] == result['bindings'][1]['events']
    assert result['bindings'][2]['events'] == []
    assert result['bindings'][3]['events'] == [{'code': 104, 'pressed': True}, {'code': 104, 'pressed': False}]
    assert result['bindings'][4]['consumerDown'] == '03 e9 00'
    assert result['bindings'][4]['consumerUp'] == '03 00 00'
    assert downs(result['dks'][3]) == [185, 75]  # release tap does not cancel after a deep press
    assert not matches
    assert result['dksModifier']['trace'][1]['modifierMaskBeforeDrain'] == 2
    assert all(e['code'] == 75 for e in result['modTap'][0]['emissionRequests'])
    assert result['modTap'][0]['emissionRequests'][-1]['pressed'] is False
    assert all(e['code'] == 185 for e in result['modTap'][1]['emissionRequests'])
    assert result['macroKinds'][0]['emissionRequests'] == []
    assert result['macroKinds'][0]['mouseButtons'][:2] == [4, 0]
    assert result['macroKinds'][1]['emissionRequests'] == [{'code': 4, 'pressed': True}, {'code': 4, 'pressed': False}]
    assert [s['simulationFlag'] for s in result['simulation']] == [1, 0]
    assert all(s['emissionRequests'] == result['bindings'][0]['events'] for s in result['simulation'])
    assert [s['emissionRequests'][0]['code'] for s in result['osMode']] == [227, 226, 226, 227]
    assert [s['consumerUsage'] for s in result['mediaAliases']] == [0xB5, 0xB6, 0xCD, 0xE2, 0xE9, 0xEA]
    result['checksPassed'] = True
    return result


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('hex', type=Path)
    p.add_argument('--output', type=Path, required=True)
    args = p.parse_args()
    result = verify(args.hex)
    args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'checksPassed': result['checksPassed'], 'output': str(args.output),
                      'dksConfigurationsSearched': result['dksExclusiveSearch']['configurations']}))
