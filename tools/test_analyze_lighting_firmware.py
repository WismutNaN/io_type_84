"""Проверяет границы анализатора; полный HEX остаётся локальным референсом."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

from analyze_lighting_firmware import analyze

ROOT = Path(__file__).resolve().parents[1]
IMAGE = ROOT / 'archive_data/firmware/2026-09-13/IO_Type_84_Magnetic_White_V1.17.hex'


class LightingFirmwareTests(unittest.TestCase):
    @unittest.skipUnless(importlib.util.find_spec('capstone'), 'optional research dependency capstone')
    def test_foreign_image_is_rejected_before_address_analysis(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / IMAGE.name
            path.write_text(':00000001FF\n', encoding='ascii')
            with self.assertRaisesRegex(ValueError, 'другой образ'):
                analyze(path)

    @unittest.skipUnless(IMAGE.exists() and importlib.util.find_spec('capstone'), 'local official HEX + capstone required')
    def test_replay_keeps_key_and_panel_address_spaces_separate(self):
        actual = analyze(IMAGE)
        expected = json.loads((ROOT / 'docs/evidence/firmware-lighting-analysis.json').read_text(encoding='utf-8'))
        self.assertEqual(actual, expected)
        key_map = actual['volatile_rgb']['slot_to_render_index']
        self.assertEqual(sorted(x for x in key_map if x != 120), list(range(84)))
        self.assertEqual(actual['panel']['render_indices'], list(range(93, 83, -1)))
        self.assertEqual(actual['panel']['reachable_via_key_rgb_map'], [])
        self.assertEqual(actual['panel']['count'], 10)
        self.assertEqual(actual['firmware_actions']['87']['target'], '0x6a14')


if __name__ == '__main__':
    unittest.main()
