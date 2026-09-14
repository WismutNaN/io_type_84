"""Validate compressed-data boundaries and compare reconstruction to original ARM code."""
from pathlib import Path
import importlib.util
import unittest

from decode_firmware_startup import decompress, load_image, initialize, emulate_initializers, report

HEX = Path(__file__).resolve().parents[1]/'archive_data/firmware/2026-09-13/IO_Type_84_Magnetic_White_V1.17.hex'


class StartupTests(unittest.TestCase):
    def test_overlapping_back_reference(self):
        # Literal A followed by a three-byte overlapping copy of the preceding byte.
        self.assertEqual(decompress(bytes([0x1A, 65, 1]), 4), (b'AAAA', 3))

    def test_corruption_cannot_escape_input_or_output(self):
        for data, length in [(b'', 1), (b'\x10', 1), (b'\x11', 0x100),
                             (b'\x19\x01', 3), (b'\x1A\x41\x01', 2),
                             (b'\x12\x41', 1), (b'\x10\x00', 1)]:
            with self.subTest(data=data, length=length), self.assertRaises(ValueError):
                decompress(data, length)

    @unittest.skipUnless(HEX.exists() and importlib.util.find_spec('unicorn'), 'local HEX and Unicorn required')
    def test_original_thumb_initializers_match_python_and_usb_identity(self):
        image = load_image(HEX)
        ram, records = initialize(image)
        actual, count = emulate_initializers(image, records)
        self.assertEqual(actual, ram)
        self.assertGreater(count, 1000)
        evidence = report(ram, records)
        self.assertEqual(evidence['usb']['interfaceCount'], 4)
        self.assertEqual(evidence['usb']['setReportCallback'], '0x1376d')
        self.assertTrue(evidence['usb']['interfaces'][2]['descriptorHex'].startswith('06 68 ff'))
        self.assertTrue(evidence['usb']['interfaces'][3]['descriptorHex'].startswith('06 67 ff'))

    @unittest.skipUnless(HEX.exists() and importlib.util.find_spec('unicorn'), 'local HEX and Unicorn required')
    def test_original_encoder_preserves_keys_and_recovers_stock_frame(self):
        from verify_panel_compositor import verify
        result = verify(HEX)
        self.assertTrue(result['checks']['changesOnlyInPanel'])
        self.assertTrue(result['checks']['stockRgbPlanesPreserved'])
        self.assertTrue(result['checks']['releaseRestoresNextEncodedFrame'])
        self.assertTrue(result['stockDiscovery']['echoOnly'])
        self.assertTrue(result['stockDiscovery']['heldMacroFlagPreserved'])
        self.assertTrue(result['stockDiscovery']['writesOnlyResponseReadyFlagsAndStack'])


if __name__ == '__main__':
    unittest.main()
