"""Runtime-tree identity compatibility and rejection controls; no project imports."""
import hashlib
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('mutie_runtime', Path(__file__).resolve().parents[2] / 'src/python/runtime.py')
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def reference(root, *, site=False):
    # Frozen pre-optimization fingerprint format, including Path sort order.
    digest = hashlib.sha256()
    for path in sorted(root.rglob('*')):
        relative = path.relative_to(root)
        if '__pycache__' in relative.parts or path.suffix in {'.pyc', '.pyo'}:
            continue
        if not site and 'site-packages' in relative.parts:
            continue
        if path.is_dir():
            continue
        name, content = str(relative).encode(), path.read_bytes()
        digest.update(len(name).to_bytes(8, 'big'))
        digest.update(name)
        digest.update(len(content).to_bytes(8, 'big'))
        digest.update(content)
    return 'sha256:' + digest.hexdigest()


class RuntimeTree(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def put(self, name, content=b'original'):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        return path

    def test_format_and_excluded_subtrees(self):
        for name in ['a/z.py', 'a.py', 'a-/b', 'a0', '.hidden', 'é.py',
                     'empty', 'dir.pyc/included.py', 'cache.pyc', 'cache.pyo',
                     '__pycache__/hidden.py', 'site-packages/package.py']:
            self.put(name, b'' if name == 'empty' else name.encode())
        for site in (False, True):
            visited = []
            scandir = os.scandir
            def observe(path):
                visited.append(Path(path))
                return scandir(path)
            with patch.object(runtime.os, 'scandir', observe):
                actual = runtime.fingerprint_tree(self.root, site=site)
            self.assertEqual(actual, reference(self.root, site=site))
            self.assertNotIn(self.root / '__pycache__', visited)
            if not site:
                self.assertNotIn(self.root / 'site-packages', visited)

    def test_same_size_backdated_edit_and_inventory_changes(self):
        path = self.put('module.py')
        original = runtime.fingerprint_tree(self.root)
        stamp = path.stat()
        path.write_bytes(b'modified')
        os.utime(path, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
        self.assertNotEqual(original, runtime.fingerprint_tree(self.root))
        path.write_bytes(b'original')
        self.assertEqual(original, runtime.fingerprint_tree(self.root))
        path.rename(self.root / 'renamed.py')
        self.assertNotEqual(original, runtime.fingerprint_tree(self.root))
        (self.root / 'renamed.py').rename(path)
        extra = self.put('new.py')
        self.assertNotEqual(original, runtime.fingerprint_tree(self.root))
        extra.unlink()
        self.assertEqual(original, runtime.fingerprint_tree(self.root))
        path.unlink()
        self.assertNotEqual(original, runtime.fingerprint_tree(self.root))

    def test_startup_hooks_and_editables_rejected(self):
        for name in ['custom.pth', 'custom.egg-link', 'sitecustomize.py', 'usercustomize.py']:
            with self.subTest(name=name):
                path = self.put('nested/' + name)
                with self.assertRaisesRegex(ValueError, 'startup hooks'):
                    runtime.fingerprint_tree(self.root, site=True)
                path.unlink()

    @unittest.skipUnless(os.name == 'posix', 'Unix links and special files')
    def test_symlinks_and_special_files(self):
        target = self.put('real.py')
        link = self.root / 'link.py'
        link.symlink_to(target)
        self.assertEqual(runtime.fingerprint_tree(self.root), reference(self.root))
        with self.assertRaisesRegex(ValueError, 'symlink unsupported'):
            runtime.fingerprint_tree(self.root, site=True)
        target.write_bytes(b'changed target')
        self.assertEqual(runtime.fingerprint_tree(self.root), reference(self.root))
        target.unlink()
        with self.assertRaises(ValueError):
            runtime.fingerprint_tree(self.root)
        link.unlink()
        directory = self.root / 'directory'
        directory.mkdir()
        link.symlink_to(directory, target_is_directory=True)
        for site in (False, True):
            with self.assertRaises(ValueError):
                runtime.fingerprint_tree(self.root, site=site)
        link.unlink()
        os.mkfifo(link)
        with self.assertRaisesRegex(ValueError, 'regular files'):
            runtime.fingerprint_tree(self.root)

    def test_dependency_requires_exact_bytes_version_purity_and_no_plugins(self):
        from types import SimpleNamespace
        self.put('webencodings/__init__.py', b'known audited bytes')
        dist = SimpleNamespace(version='0.5.1', requires=[], entry_points=[],
                               read_text=lambda name: 'Root-Is-Purelib: true')
        expected = runtime.fingerprint_tree(self.root / 'webencodings', site=True)
        with patch.object(runtime, 'WEBENCODINGS', expected):
            runtime.validate_application_dependency(self.root, dist)
            for field, value in [('version','0.5.2'), ('requires',['other']),
                                 ('entry_points',['plugin']), ('read_text',lambda name:'Root-Is-Purelib: false')]:
                old = getattr(dist, field)
                setattr(dist, field, value)
                with self.assertRaises(ValueError): runtime.validate_application_dependency(self.root, dist)
                setattr(dist, field, old)
            for filename in ('webencodings/extra.py', 'webencodings/native.so', 'webencodings/startup.pth'):
                file = self.put(filename)
                with self.assertRaises(ValueError): runtime.validate_application_dependency(self.root, dist)
                file.unlink()
            self.put('webencodings/__init__.py', b'changed bytes')
            with self.assertRaisesRegex(ValueError, 'bytes differ'):
                runtime.validate_application_dependency(self.root, dist)

    def test_scan_errors_propagate(self):
        with patch.object(runtime.os, 'scandir', side_effect=PermissionError('denied')):
            with self.assertRaises(PermissionError):
                runtime.fingerprint_tree(self.root)


if __name__ == '__main__':
    unittest.main()
