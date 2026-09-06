from __future__ import annotations

import ast
from pathlib import Path
import unittest


class IndependenceTests(unittest.TestCase):
    def test_runtime_modules_have_no_product_imports(self) -> None:
        root = Path(__file__).parents[1]
        forbidden_prefixes = ("apps", "backend", "orb_relay", "company")
        for path in root.rglob("*.py"):
            if path.name.startswith("test_"):
                continue
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            imported: list[str] = []
            for node in ast.walk(tree):
                if isinstance(node, ast.Import):
                    imported.extend(alias.name for alias in node.names)
                elif isinstance(node, ast.ImportFrom) and node.module:
                    imported.append(node.module)
            for module in imported:
                self.assertFalse(
                    module.startswith(forbidden_prefixes),
                    f"{path.name} imports product module {module}",
                )


if __name__ == "__main__":
    unittest.main()
