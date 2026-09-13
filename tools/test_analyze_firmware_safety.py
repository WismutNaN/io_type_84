"""Evidence consistency, input boundaries, and independent vendor-loader comparison."""
import json
from pathlib import Path
import tempfile
import unittest

from analyze_firmware_safety import analyze, canonical_payload, USER_ROM_BYTES

ROOT = Path(__file__).resolve().parents[1]
HEX = ROOT/'archive_data/firmware/2026-09-13/IO_Type_84_Magnetic_White_V1.17.hex'
PAYLOAD = ROOT/'docs/evidence/firmware-isp-payload.json'

class FirmwareSafetyTests(unittest.TestCase):
    def test_wrong_image_cannot_produce_address_claims(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory)/'foreign.hex'
            file.write_text(':00000001FF\n', encoding='ascii')
            with self.assertRaisesRegex(ValueError, 'Unreviewed'):
                analyze(file)

    def test_external_regions_are_not_silently_dropped(self):
        for address in [-1, USER_ROM_BYTES, 0x00400000]:
            with self.subTest(address=address), self.assertRaises(ValueError):
                canonical_payload({address: 0})

    @unittest.skipUnless(HEX.exists() and PAYLOAD.exists(), 'local official image and offline WASM evidence')
    def test_independent_payload_matches_actual_vendor_loader(self):
        vendor = json.loads(PAYLOAD.read_text(encoding='utf-8'))
        result = analyze(HEX)
        self.assertEqual(result['canonicalPayload']['sha256'], vendor['payloadSha256'])
        self.assertEqual(result['canonicalPayload']['sum16LittleEndian'], vendor['vendorLoaderReturn'][0])
        self.assertEqual(result['canonicalPayload']['length'], vendor['programLength'])
        self.assertEqual(vendor['blockedImports'], [])
        self.assertEqual(result['calibration']['maxValues'], {2200: 128})
        self.assertEqual(result['calibration']['minValues'], {1900: 128})
        self.assertEqual(result['ispEntry']['bootRomThumbTarget'], '0x200301')

if __name__ == '__main__':
    unittest.main()
