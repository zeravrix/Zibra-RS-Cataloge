import unittest
from pathlib import Path
import extract_kotatsu as e

FX = Path(__file__).parent / "fixtures" / "kotatsu"


class ExtractTest(unittest.TestCase):
    def test_keeps_clean_skips_rest(self):
        recs, skipped = e.extract(FX)
        names = {r["name"] for r in recs}
        self.assertEqual(names, {"TOONILY", "AZORAMOON", "MANHWACLAN", "DRAGONTEA"})
        self.assertEqual(skipped, {"adult": 2, "custom": 1, "broken": 1})
        toonily = next(r for r in recs if r["name"] == "TOONILY")
        self.assertEqual(toonily["list_path"], "webtoon")
        self.assertEqual(toonily["domain"], "toonily.com")


if __name__ == "__main__":
    unittest.main()
