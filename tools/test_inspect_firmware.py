import unittest

from inspect_firmware import inspect, parse_hex


def record(address: int, kind: int, data: bytes = b"") -> str:
    body = bytes([len(data)]) + address.to_bytes(2, "big") + bytes([kind]) + data
    return ":" + (body + bytes([-sum(body) & 0xFF])).hex().upper()


EOF = record(0, 1)


class HexTests(unittest.TestCase):
    def test_sparse_linear_addresses_are_not_flattened_into_fake_data(self):
        source = "\n".join([record(0, 0, b"AB"), record(0, 4, b"\x00\x07"), record(0xDFFE, 0, b"CD"), EOF])
        result = inspect(source.encode())
        self.assertEqual(result["data_bytes"], 4)
        self.assertEqual(result["address_span_bytes"], 0x7E000)
        self.assertEqual(len(result["segments"]), 2)

    def test_wrong_checksum_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "контрольная сумма"):
            parse_hex(":010000004100\n" + EOF)

    def test_truncated_record_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "длина"):
            parse_hex(":0200000041BD\n" + EOF)

    def test_conflicting_overlap_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "перекрывающихся"):
            parse_hex("\n".join([record(0, 0, b"A"), record(0, 0, b"B"), EOF]))

    def test_missing_or_premature_eof_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "EOF"):
            parse_hex(record(0, 0, b"A"))
        with self.assertRaisesRegex(ValueError, "после EOF"):
            parse_hex("\n".join([EOF, record(0, 0, b"A")]))

    def test_segment_address_and_start_linear_address(self):
        memory, metadata = parse_hex("\n".join([
            record(0, 2, b"\x12\x34"), record(2, 0, b"Z"),
            record(0, 5, bytes.fromhex("00012343")), EOF,
        ]))
        self.assertEqual(memory, {0x12342: ord("Z")})
        self.assertEqual(metadata["entry_address"], "0x12343")


if __name__ == "__main__":
    unittest.main()
