"""Воспроизводимый статический разбор IO White 1.17. USB и запись HEX отсутствуют."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
from inspect_firmware import parse_hex

EXPECTED_SHA256 = "8f5ef507771c6795258eb7521cfc1b46269a5b301548767ae87a3f1a83a50d45"

def analyze(path: Path) -> dict:
    from capstone import Cs, CS_ARCH_ARM, CS_MODE_THUMB, CS_MODE_LITTLE_ENDIAN
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != EXPECTED_SHA256:
        raise ValueError("Адреса проверены только для точного IO White V1.17; другой образ отклонён.")
    memory, _ = parse_hex(raw.decode("ascii"))
    def data(start, end):
        return bytes(memory[a] for a in range(start, end))
    def word(address):
        return int.from_bytes(data(address, address + 4), "little")
    decoder = Cs(CS_ARCH_ARM, CS_MODE_THUMB | CS_MODE_LITTLE_ENDIAN)
    def disassembly(start, end):
        result = []
        for ins in decoder.disasm(data(start, end), start):
            row = {"address": hex(ins.address), "bytes": ins.bytes.hex(" "), "instruction": f"{ins.mnemonic} {ins.op_str}".strip()}
            if ins.mnemonic.startswith("ldr") and "[pc, #" in ins.op_str:
                offset = int(ins.op_str.split("[pc, #")[1].split("]")[0], 0)
                address = ((ins.address + 4) & ~3) + offset
                row["literal_address"] = hex(address)
                row["literal_value"] = hex(word(address))
            result.append(row)
        return result
    key_map = list(data(0x152d5, 0x15355))
    panel_map = list(data(0x155bb, 0x155cf))[::2]
    functions = {}
    for code in (27, 87, 91):
        entry = 0x374c + code * 4
        stub = 0x374c + word(entry)
        ins = next(decoder.disasm(data(stub, stub + 4), stub))
        if ins.mnemonic != "b.w":
            raise ValueError("Неожиданная таблица firmware actions")
        functions[str(code)] = {"table_entry": hex(entry), "stub": hex(stub), "target": ins.op_str.removeprefix("#")}
    return {
        "image_sha256": EXPECTED_SHA256,
        "method": "Intel HEX checksums + Thumb disassembly; no execution, USB or firmware writes",
        "command_dispatcher": "0xd7c4",
        "observed_get_cases": ["10", "11", "12", "13", "14", "15", "16", "17", "18", "1c"],
        "observed_set_cases": ["21", "22", "23", "24", "25", "26", "27", "28"],
        "unsupported_in_this_dispatcher": ["1b", "2b", "2a", "2d", "33", "60", "68"],
        "volatile_rgb": {
            "command": "32", "handler": "0xdcc8", "tuple": ["matrixSlot", "channel0", "channel1", "channel2"],
            "count": "floor(request[2] / 4)", "map_address": "0x152d5", "slot_to_render_index": key_map,
            "skip_render_index": 120, "channel_arrays": ["0x20005514", "0x20005592", "0x20005610"],
            "channel_pitch": 126, "scale": "floor(((channel * brightnessByte * 255) >> 16) / 6)",
            "brightness_address": "0x200002c9", "sets_ram": {"0x2000036b": 1, "0x2000009a": 50},
            "calls_in_handler": [],
        },
        "panel": {
            "render_map_address": "0x155bb", "render_indices": panel_map,
            "count_address": "0x155cf", "count": memory[0x155cf],
            "private_rgb_arrays": ["0x20005d8e", "0x20005d98", "0x20005da2"],
            "envelope_array": "0x20005dac", "state_base": "0x200002f0",
            "compositor": "0xeea0", "mode_dispatcher": "0xe540", "depth_renderer": "0xeb60",
            "depth_thresholds_address": "0x15603", "depth_thresholds": list(data(0x15603,0x1560d)),
            "depth_input_address": "0x20000108", "threshold_unit": "requires physical correlation",
            "reachable_via_key_rgb_map": sorted(set(panel_map) & set(key_map)),
        },
        "firmware_actions": functions,
        "regions": {name: disassembly(a,b) for name,a,b in [
            ("get_dispatch",0xd82c,0xd85e), ("set_dispatch",0xd948,0xd97a),
            ("simulation_dispatch",0xdaf2,0xdb2c), ("rgb_dispatch",0xdb2c,0xdb38),
            ("volatile_rgb",0xdcc8,0xdd3e), ("func_dispatch",0x3738,0x374a),
            ("panel_depth_toggle",0x6a14,0x6a3a), ("panel_mode_cycle",0x68e4,0x691e),
            ("panel_overlay_cycle",0x67f4,0x6826), ("depth_input",0x3a84,0x3a8a),
            ("depth_fill",0xeb60,0xebfc), ("panel_compositor_start",0xeea0,0xeef8),
            ("panel_compositor_colors",0xf066,0xf0c4), ("led_output",0x13028,0x13052),
        ]},
        "limits": ["Static reachability is not hardware verification.", "Panel wiring/order, RGB order and scaling need a reversible physical test.", "No proven volatile release/restore sequence for command 32 yet.", "No firmware patch, animation upload or extra-layer format was established."],
    }

if __name__ == "__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("hex",type=Path)
    parser.add_argument("--output",type=Path,required=True)
    args=parser.parse_args()
    report=analyze(args.hex)
    args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(f"Сохранён статический отчёт: {args.output}")
