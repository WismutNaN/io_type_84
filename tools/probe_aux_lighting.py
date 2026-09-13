"""Read three fixed IO White 1.17 blocks; never SET/reset/calibrate/enter OTA.

Run with: uv run --with hidapi python tools/probe_aux_lighting.py
Uses the previously verified 64-byte FF68 collection. Stop on any mismatch or
timeout; do not retry or continue a desynchronised session. Output omits OS paths
and serial numbers. A valid response is not proof of a physical lighting effect.
"""

import datetime
import json
import sys

VID, PID = 0x0C45, 0x80D6
PRODUCT = "IO Type 84 Magnetic White"
DESCRIPTOR = bytes.fromhex(
    "06 68 ff 09 61 a1 01 09 62 15 00 26 ff 00 75 08 95 40 81 02 "
    "09 63 15 00 26 ff 00 75 08 95 40 91 02 c0"
)
READS = ((0x10, 48, "device_info"), (0x1B, 24, "light_box"), (0x1D, 24, "side_light"))


def request(cmd, size):
    if (cmd, size) not in [(c, s) for c, s, _ in READS]:
        raise ValueError("Outside fixed read allowlist")
    frame = bytearray(64)
    frame[:8] = bytes([0xAA, cmd, size, 0, 0, 0, 1, 0])
    return bytes(frame)


def validate_response(sent, received):
    if len(received) != 64:
        raise ValueError(f"Expected 64-byte reply, received {len(received)}")
    if received[0] != 0x55 or received[1:8] != sent[1:8]:
        raise ValueError("Unexpected reply header; session stopped")
    return received[8:8 + sent[2]]


def validate_identity(payload):
    # Pin the observed model, firmware and frame version before new GETs.
    expected = {
        (4, 8): bytes.fromhex("45 0c d6 80"),
        (8, 10): bytes.fromhex("17 01"),
        (12, 16): bytes.fromhex("66 01 0c 11"),
        (30, 31): b"\x00",
    }
    if len(payload) != 48 or any(payload[a:b] != value for (a, b), value in expected.items()):
        raise ValueError("Different device/firmware; review required before additional GETs")


def main():
    import hid

    result = {
        "captured_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "vid": "0C45", "pid": "80D6", "product": PRODUCT,
        "method": "Native hidapi; fixed GET 10/1B/1D; exact descriptor and identity; no retries",
        "packets": [], "configuration_writes": 0,
        "physical_panel_mapping_verified": False,
    }
    device = None
    try:
        candidates = [
            d for d in hid.enumerate(VID, PID)
            if d["usage_page"] == 0xFF68 and d["usage"] == 0x61
            and d["interface_number"] == 2 and (d["product_string"] or "").strip() == PRODUCT
        ]
        if len(candidates) != 1:
            raise ValueError(f"Expected one matching FF68 collection, found {len(candidates)}")
        device = hid.device()
        device.open_path(candidates[0]["path"])
        if bytes(device.get_report_descriptor()) != DESCRIPTOR:
            raise ValueError("Descriptor changed; review required")
        for cmd, size, name in READS:
            sent = request(cmd, size)
            row = {"name": name, "cmd": f"0x{cmd:02X}", "request_hex": sent.hex(" ")}
            result["packets"].append(row)
            written = device.write(b"\x00" + sent)
            row["hidapi_bytes_written"] = written
            if written != 65:
                raise ValueError("Short HID output report")
            received = bytes(device.read(64, timeout_ms=1000))
            row["response_hex"] = received.hex(" ")
            payload = validate_response(sent, received)
            row["payload_hex"] = payload.hex(" ")
            row["validated"] = True
            if cmd == 0x10:
                validate_identity(payload)
            else:
                row["all_zero"] = not any(payload)
                row["candidate_fields_from_vendor_sdk"] = {
                    "mode": payload[0], "rgb": list(payload[1:4]),
                    "color_mode": payload[8], "brightness": payload[9], "speed": payload[10],
                }
        result["status"] = "completed"
    except Exception as error:
        result["status"] = "stopped"
        # hidapi errors may contain host identifiers; keep only our validation
        # messages for ValueError, otherwise the exception class.
        result["error"] = str(error) if isinstance(error, ValueError) else type(error).__name__
    finally:
        if device is not None:
            device.close()
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if result["status"] == "completed" else 1


if __name__ == "__main__":
    sys.exit(main())
