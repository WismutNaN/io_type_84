"""Exercise fail-closed boundaries without importing hidapi or opening hardware."""

import unittest

from probe_aux_lighting import request, validate_identity, validate_response


class ProbeBoundaryTests(unittest.TestCase):
    def test_write_and_mode_commands_rejected(self):
        for cmd in (0x0F, 0x2B, 0x32, 0x66, 0x82):
            with self.subTest(cmd=cmd), self.assertRaises(ValueError):
                request(cmd, 24)

    def test_wrong_read_length_rejected(self):
        with self.assertRaises(ValueError):
            request(0x1B, 56)

    def test_timeout_and_wrong_headers_rejected(self):
        sent = request(0x1B, 24)
        valid = b"\x55" + sent[1:]
        self.assertEqual(validate_response(sent, valid), bytes(24))
        for response in (b"", valid[:32], sent):
            with self.subTest(response=response), self.assertRaises(ValueError):
                validate_response(sent, response)
        for index in range(1, 8):
            changed = bytearray(valid)
            changed[index] ^= 1
            with self.subTest(index=index), self.assertRaises(ValueError):
                validate_response(sent, bytes(changed))

    def test_other_model_firmware_and_frame_version_rejected(self):
        payload = bytearray(48)
        payload[4:10] = bytes.fromhex("45 0c d6 80 17 01")
        payload[12:16] = bytes.fromhex("66 01 0c 11")
        validate_identity(payload)
        for index in (6, 8, 12, 14, 30):
            changed = payload.copy()
            changed[index] ^= 1
            with self.subTest(index=index), self.assertRaises(ValueError):
                validate_identity(changed)


if __name__ == "__main__":
    unittest.main()
