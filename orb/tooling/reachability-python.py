#!/usr/bin/env python3
"""Python half of the Focus Orb production-reachability census.

The Node entry point owns reporting.  This helper uses only the standard-library AST so Python
imports, public callables, Pydantic wire models, and suppression comments are parsed as Python
rather than approximated with JavaScript regular expressions.
"""

from __future__ import annotations

import argparse
import ast
import json
import re
from collections import defaultdict
from pathlib import Path
from typing import Any


TEST_PARTS = {"tests", "test", "fixtures", "fixture"}
TEST_NAME_RE = re.compile(r"(^test_|_test\.py$|\.test\.py$)")
NOQA_RE = re.compile(r"#\s*noqa(?::\s*[A-Z0-9, ]+)?(?P<reason>.*)$", re.IGNORECASE)
TYPE_IGNORE_RE = re.compile(r"#\s*type:\s*ignore(?:\[[^]]+\])?(?P<reason>.*)$", re.IGNORECASE)
REASON_RE = re.compile(r"(?:--|(?:^|\s)-\s|reason:)\s*\S", re.IGNORECASE)


def rel(root: Path, path: Path) -> str:
    return path.resolve().relative_to(root.resolve()).as_posix()


def is_test_file(path: Path) -> bool:
    return bool(TEST_PARTS.intersection(path.parts) or TEST_NAME_RE.search(path.name))


def python_files(root: Path, paths: list[str]) -> list[Path]:
    result: set[Path] = set()
    for configured in paths:
        base = root / configured
        if base.is_file() and base.suffix == ".py":
            result.add(base)
        elif base.is_dir():
            result.update(
                path
                for path in base.rglob("*.py")
                if not {".venv", "__pycache__", "site-packages"}.intersection(path.parts)
            )
    return sorted(result)


def module_name(path: Path, source_roots: list[Path]) -> str | None:
    for source_root in sorted(source_roots, key=lambda item: len(item.parts), reverse=True):
        try:
            relative = path.relative_to(source_root)
        except ValueError:
            continue
        parts = list(relative.with_suffix("").parts)
        if parts and parts[-1] == "__init__":
            parts.pop()
        return ".".join(parts)
    return None


def parent_links(tree: ast.AST) -> None:
    for parent in ast.walk(tree):
        for child in ast.iter_child_nodes(parent):
            setattr(child, "_reachability_parent", parent)


def inside_declaration(node: ast.AST, declaration: ast.AST) -> bool:
    current: ast.AST | None = node
    while current is not None:
        if current is declaration:
            return True
        current = getattr(current, "_reachability_parent", None)
    return False


def reason_present(text: str) -> bool:
    return bool(REASON_RE.search(text))


def suppression_findings(root: Path, files: list[Path]) -> tuple[list[dict[str, Any]], int]:
    findings: list[dict[str, Any]] = []
    found = 0
    for path in files:
        if is_test_file(path):
            continue
        for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for kind, pattern in (("noqa", NOQA_RE), ("type-ignore", TYPE_IGNORE_RE)):
                match = pattern.search(line)
                if not match:
                    continue
                found += 1
                reason = match.group("reason") or ""
                if reason_present(reason):
                    continue
                file_name = rel(root, path)
                findings.append(
                    {
                        "id": f"suppression:{file_name}:{line_number}:{kind}",
                        "code": "suppression-missing-reason",
                        "file": file_name,
                        "line": line_number,
                        "symbol": kind,
                        "message": f"{kind} suppression has no justification after `--`, ` - `, or `reason:`",
                    }
                )
    return findings, found


def callable_declarations(tree: ast.Module, module: str, path: Path) -> dict[str, dict[str, Any]]:
    declarations: dict[str, dict[str, Any]] = {}
    for node in tree.body:
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            continue
        if node.name.startswith("_"):
            continue
        key = f"{module}:{node.name}"
        declarations[key] = {
            "key": key,
            "module": module,
            "symbol": node.name,
            "path": path,
            "node": node,
            "prod_files": set(),
            "test_files": set(),
            "references": 0,
        }
    return declarations


def scan_symbols(
    root: Path,
    source_files: list[Path],
    test_files: list[Path],
    source_roots: list[Path],
) -> tuple[list[dict[str, Any]], int, int]:
    trees: dict[Path, ast.Module] = {}
    modules: dict[str, Path] = {}
    declarations: dict[str, dict[str, Any]] = {}

    for path in [*source_files, *test_files]:
        try:
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        except SyntaxError as error:
            raise RuntimeError(f"cannot parse {rel(root, path)}:{error.lineno}: {error.msg}") from error
        parent_links(tree)
        trees[path] = tree
        module = module_name(path, source_roots)
        if module is not None and path in source_files:
            modules[module] = path
            declarations.update(callable_declarations(tree, module, path))

    for path, tree in trees.items():
        current_module = module_name(path, source_roots)
        imported_names: dict[str, str] = {}
        imported_modules: dict[str, str] = {}
        for node in ast.walk(tree):
            if isinstance(node, ast.ImportFrom) and node.module:
                imported_module = node.module
                if node.level and current_module:
                    package_parts = current_module.split(".")[:-1]
                    keep = max(0, len(package_parts) - (node.level - 1))
                    imported_module = ".".join([*package_parts[:keep], *node.module.split(".")])
                for alias in node.names:
                    key = f"{imported_module}:{alias.name}"
                    if key in declarations:
                        imported_names[alias.asname or alias.name] = key
            elif isinstance(node, ast.Import):
                for alias in node.names:
                    if alias.name in modules:
                        imported_modules[alias.asname or alias.name.split(".")[0]] = alias.name

        for node in ast.walk(tree):
            key: str | None = None
            if isinstance(node, ast.Name) and isinstance(node.ctx, ast.Load):
                key = imported_names.get(node.id)
                if key is None and current_module is not None:
                    candidate = f"{current_module}:{node.id}"
                    if candidate in declarations:
                        key = candidate
            elif isinstance(node, ast.Attribute) and isinstance(node.ctx, ast.Load):
                if isinstance(node.value, ast.Name):
                    imported_module = imported_modules.get(node.value.id)
                    candidate = f"{imported_module}:{node.attr}" if imported_module else None
                    if candidate in declarations:
                        key = candidate

            if key is None:
                continue
            declaration = declarations[key]
            if path == declaration["path"] and inside_declaration(node, declaration["node"]):
                continue
            bucket = "test_files" if is_test_file(path) else "prod_files"
            declaration[bucket].add(rel(root, path))
            declaration["references"] += 1

    findings: list[dict[str, Any]] = []
    references_resolved = 0
    for declaration in declarations.values():
        references_resolved += declaration["references"]
        if declaration["prod_files"] or not declaration["test_files"]:
            continue
        file_name = rel(root, declaration["path"])
        symbol = declaration["symbol"]
        findings.append(
            {
                "id": f"orphan:{file_name}:{symbol}",
                "code": "exported-test-only",
                "file": file_name,
                "line": declaration["node"].lineno,
                "symbol": symbol,
                "message": (
                    f"production_importers=0 test_importers={len(declaration['test_files'])}; "
                    "public callable is reached only from tests/fixtures"
                ),
            }
        )
    return findings, len(declarations), references_resolved


def class_fields(root: Path, contract: dict[str, Any]) -> dict[str, Any]:
    path = root / contract["pythonFile"]
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    wanted = {contract["requestClass"], contract["responseClass"]}
    result: dict[str, Any] = {}
    for node in tree.body:
        if not isinstance(node, ast.ClassDef) or node.name not in wanted:
            continue
        fields: list[str] = []
        required: list[str] = []
        for statement in node.body:
            if not isinstance(statement, ast.AnnAssign) or not isinstance(statement.target, ast.Name):
                continue
            fields.append(statement.target.id)
            if statement.value is None:
                required.append(statement.target.id)
        result[node.name] = {"fields": fields, "required": required, "line": node.lineno}
    missing = wanted.difference(result)
    if missing:
        raise RuntimeError(
            f"wire contract {contract['name']} missing Python models in {contract['pythonFile']}: "
            + ", ".join(sorted(missing))
        )
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--config", required=True)
    args = parser.parse_args()
    root = Path(args.root).resolve()
    config = json.loads(Path(args.config).read_text(encoding="utf-8"))
    python_config = config.get("python", {})
    source_roots = [root / item for item in python_config.get("sourceRoots", [])]
    test_roots = [root / item for item in python_config.get("testRoots", [])]
    source_files = python_files(root, python_config.get("sourceRoots", []))
    test_files = python_files(root, python_config.get("testRoots", []))

    symbol_findings, symbols_scanned, references_resolved = scan_symbols(
        root, source_files, test_files, source_roots
    )
    suppression_results, suppressions_found = suppression_findings(
        root, [*source_files, *test_files]
    )
    models = {
        contract["name"]: class_fields(root, contract)
        for contract in config.get("wireContracts", [])
    }
    print(
        json.dumps(
            {
                "filesScanned": len(source_files) + len(test_files),
                "productionFiles": len(source_files),
                "symbolsScanned": symbols_scanned,
                "referencesResolved": references_resolved,
                "suppressionsFound": suppressions_found,
                "findings": [*symbol_findings, *suppression_results],
                "models": models,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
