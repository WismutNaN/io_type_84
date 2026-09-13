"""Offline comparison of reviewed reference functions with IO capture fixtures.

Requires Python 3.10+, rustc and the two local reference checkouts. Never opens
HID or imports the reference applications. Hashes pin the reviewed source bytes;
changed upstream files require a new review, not automatic execution.
"""

import ast
import hashlib
import json
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
ARCHIVE = ROOT / "archive_data/reference_projects"
MINI_PATH = ARCHIVE / "Aether-HE/protocol_mini60.py"
AK_PATH = ARCHIVE / "ak820pro-modder/crates/ak820-protocol/src/protocol.rs"
MINI_HASH = "68fc4f3a575557bfd24e593ee8dfb1adc2ec7fbd54181e2837051c57a12bd7b8"
AK_HASH = "88cae171734661c384193ec3c0da7d4b88646f39f71697c5bb326bf874988d1f"


def reviewed_source(path, expected_hash):
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != expected_hash:
        raise ValueError(f"Review required: changed source {path}")
    return data.decode("utf-8")


def mini_functions(source):
    """Select only reviewed pure codec functions and literal constants."""
    functions = {
        "page_size", "_frame", "build_table_read", "mm_to_raw", "raw_to_mm",
        "_act_record", "parse_actuation_records",
    }
    constants = {"MAGIC", "FRAME_LEN", "PAYLOAD_START", "ACT_RECORD_SIZE", "TRIGGER_UNIT_MM"}
    selected = []
    namespace = {}
    for node in ast.parse(source).body:
        if isinstance(node, ast.Assign) and len(node.targets) == 1:
            target = node.targets[0]
            if isinstance(target, ast.Name) and target.id in constants:
                namespace[target.id] = ast.literal_eval(node.value)
        if isinstance(node, ast.FunctionDef) and node.name in functions:
            selected.append(node)
    if len(selected) != len(functions) or not constants <= namespace.keys():
        raise ValueError("Reviewed declarations missing")
    exec(compile(ast.Module(body=selected, type_ignores=[]), str(MINI_PATH), "exec"), namespace)
    return namespace


def compare_ak(source, packets):
    """Compile only build_frame; no crate/build.rs/dependencies/device code."""
    start = source.index("pub fn build_frame(")
    end = source.index("\n}", start) + 2
    function = source[start:end]
    harness = "const PACKET_LEN: usize = 64; const HEADER_LEN: usize = 8;\n"
    harness += "const PAYLOAD_PER_PACKET: usize = 56; const MAGIC_OUTGOING: u8 = 0xaa;\n"
    harness += function + "\nfn main() {\n"
    for packet in packets:
        frame = bytes.fromhex(packet["request_hex"])
        expected = ",".join(map(str, frame))
        payload = ",".join(map(str, frame[8:8 + frame[2]]))
        harness += (
            f"assert_eq!(build_frame({frame[1]}, {frame[2]}, "
            f"{int.from_bytes(frame[3:5], 'little')}, &[{payload}], "
            f"{'true' if frame[6] else 'false'}), [{expected}]);\n"
        )
    harness += "}\n"
    with tempfile.TemporaryDirectory(prefix="io-codec-") as directory:
        test = pathlib.Path(directory) / "reference_frames.rs"
        executable = pathlib.Path(directory) / "reference_frames.exe"
        test.write_text(harness, encoding="utf-8")
        subprocess.run(["rustc", "--edition=2021", str(test), "-o", str(executable)], check=True)
        subprocess.run([str(executable)], check=True)
    return len(packets)


def main():
    snapshot_path = ROOT / "docs/evidence/read-snapshot.json"
    snapshot = json.loads(snapshot_path.read_text(encoding="utf-8"))
    mini = mini_functions(reviewed_source(MINI_PATH, MINI_HASH))
    ak_matches = compare_ak(reviewed_source(AK_PATH, AK_HASH), snapshot["packets"])
    mini_matches = 0
    rt_decoded = 0
    rt_valid = 0
    for packet in snapshot["packets"]:
        frame = bytes.fromhex(packet["request_hex"])
        if not any(frame[8:]):
            actual = mini["build_table_read"](
                frame[1], int.from_bytes(frame[3:5], "little"), frame[2], bool(frame[6])
            )
            if bytes(actual) != frame:
                raise ValueError(f"Mini60 request mismatch: {packet['cmd']}:{packet['address']}")
            mini_matches += 1
        if frame[1] == 0x17:
            response = bytes.fromhex(packet["response_hex"])
            records = mini["parse_actuation_records"](response)
            rt_decoded += len(records)
            # The upstream parser uses frame capacity, not declared payload size.
            # Discard padding in the final 16-byte IO page explicitly here.
            for index, record in enumerate(records[:response[2] // 8]):
                encoded = bytes(mini["_act_record"](*record)) if record else bytes(8)
                if encoded != response[8 + index * 8:16 + index * 8]:
                    raise ValueError("RT round-trip mismatch")
                rt_valid += 1
    info = bytes.fromhex(snapshot["blocks"]["device_info"]["hex"])
    report = {
        "captured_at": "2026-09-13",
        "method": "Offline execution of hash-pinned pure codec functions on existing IO fixtures; no HID",
        "sources": [
            {"url": "https://github.com/wsclx/ak820pro-modder", "commit": "fe93c13972445f92b00b485875da2811173eb940", "file": "crates/ak820-protocol/src/protocol.rs", "sha256": AK_HASH},
            {"url": "https://github.com/MrWhosNexus/Aether-HE", "commit": "2179fd768fcdbb86f59c4c240b4d4cfcbc18936f", "file": "protocol_mini60.py", "sha256": MINI_HASH},
        ],
        "fixture_sha256": hashlib.sha256(snapshot_path.read_bytes()).hexdigest(),
        "ak_build_frame": {"matched": ak_matches, "total": len(snapshot["packets"])},
        "mini_build_table_read": {"matched": mini_matches, "excluded": "0x68 carries key IDs; not an empty table-read request"},
        "mini_actuation_round_trip": {"valid_records": rt_valid, "untrimmed_parser_records": rt_decoded, "rt_precision": info[29]},
        "macro_space": {"raw_bytes_2_3_hex": info[2:4].hex(" "), "raw_le16": int.from_bytes(info[2:4], "little"), "website_profile_value": 512, "actual_usable_capacity_verified": False},
        "limits": "Tests prove framing and RT record compatibility for captured reads only; no SET, firmware or whole-application compatibility claim.",
    }
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
