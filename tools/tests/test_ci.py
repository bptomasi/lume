"""Test compiler ownership, feature scenarios and mandatory release gates."""

import contextlib
import importlib
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
with patch.object(sys, 'path', [str(TOOLS), *sys.path]):
    ci = importlib.import_module('ci')


class CompilerTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix='lume-compiler-')
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        (self.root / 'rust-toolchain.toml').write_text('''
            [toolchain]
            channel = "1.99.1"
            profile = "minimal"
            components = ["clippy", "rustfmt"]
        ''')
        (self.root / 'Cargo.toml').write_text('''
            [package]
            rust-version = "1.96"
        ''')

    def test_pinned_version_profile_and_components_follow_file(self):
        configuration = ci.compiler_configuration(self.root, 'pinned')
        self.assertEqual(ci.install_command(configuration), [
            'rustup', 'toolchain', 'install', '1.99.1', '--profile', 'minimal',
            '--component', 'clippy', '--component', 'rustfmt',
        ])
        path = self.root / 'rust-toolchain.toml'
        path.write_text(path.read_text().replace('1.99.1', '1.98.2')
                        .replace('"clippy", ', ''))
        self.assertEqual(ci.compiler_configuration(self.root, 'pinned'),
                         ('1.98.2', 'minimal', ['rustfmt']))

    def test_minimum_follows_manifest_and_preserves_patch(self):
        self.assertEqual(ci.compiler_configuration(self.root, 'minimum'),
                         ('1.96.0', 'minimal', []))
        (self.root / 'Cargo.toml').write_text(
            '[package]\nrust-version = "1.96.2"\n',
        )
        self.assertEqual(ci.compiler_configuration(self.root, 'minimum')[0],
                         '1.96.2')

    def test_moving_release_channel_is_rejected(self):
        path = self.root / 'rust-toolchain.toml'
        path.write_text(path.read_text().replace('1.99.1', 'stable'))
        with self.assertRaisesRegex(ValueError, 'exact Rust version'):
            ci.compiler_configuration(self.root, 'pinned')

    def test_cache_identity_uses_the_selected_actual_compiler(self):
        output = self.root / 'github-output'
        fingerprints = []
        for commit in ('first-build', 'second-build'):
            output.write_text('existing=value\n')
            identity = f'rustc 1.99.1\ncommit-hash: {commit}\n'
            result = subprocess.CompletedProcess([], 0, stdout=identity)
            with (patch.object(ci.subprocess, 'run',
                               return_value=result) as run,
                  patch.dict(os.environ, GITHUB_OUTPUT=str(output)),
                  contextlib.redirect_stdout(io.StringIO())):
                ci.install(('1.99.1', 'minimal', []), self.root)
            self.assertEqual(run.call_args_list[1].args[0],
                             ['rustc', '+1.99.1', '-Vv'])
            lines = output.read_text().splitlines()
            self.assertEqual(lines[0], 'existing=value')
            self.assertEqual(lines[2], 'version=1.99.1')
            fingerprints.append(lines[1])
        self.assertNotEqual(*fingerprints)

    def test_failed_install_does_not_publish_a_cache_identity(self):
        output = self.root / 'github-output'
        failure = subprocess.CalledProcessError(1, ['rustup'])
        with (patch.object(ci.subprocess, 'run', side_effect=failure) as run,
              patch.dict(os.environ, GITHUB_OUTPUT=str(output))):
            with self.assertRaises(subprocess.CalledProcessError):
                ci.install(('1.99.1', 'minimal', []), self.root)
        self.assertEqual(run.call_count, 1)
        self.assertFalse(output.exists())

    def test_minimum_runs_actual_tests_with_its_explicit_compiler(self):
        commands = ci.qualification_commands('1.96.0', 'minimum', 'minimal')
        self.assertEqual(commands, [[
            'cargo', '+1.96.0', 'test', '--locked', '--all-targets',
            '--no-default-features', '--', '--test',
        ]])

    def test_pinned_profiles_require_strict_clippy_and_test_correctness(self):
        for profile, features in ci.PROFILES.items():
            with self.subTest(profile=profile):
                commands = ci.qualification_commands(
                    '1.99.1', 'pinned', profile, offline=True,
                )
                self.assertEqual(len(commands), 2)
                self.assertEqual(commands[0][:3],
                                 ['cargo', '+1.99.1', 'clippy'])
                self.assertEqual(commands[0][-3:], ['--', '-D', 'warnings'])
                self.assertEqual(commands[1][-2:], ['--', '--test'])
                for command in commands:
                    self.assertIn('--offline', command)
                    self.assertIn('--all-targets', command)
                    for feature in features:
                        self.assertIn(feature, command)

    def test_profiles_cover_every_manifest_feature_and_independent_sixel(self):
        manifest = tomllib.loads((TOOLS.parent / 'Cargo.toml').read_text())
        covered = set()
        for options in ci.PROFILES.values():
            if '--features' in options:
                selected = options[options.index('--features') + 1]
                covered.update(selected.split(','))
        self.assertEqual(covered, set(manifest['features']) - {'default'})
        self.assertEqual(ci.PROFILES['sixel'],
                         ('--no-default-features', '--features', 'sixel'))

    def test_workflow_uses_sources_and_all_jobs_block_release(self):
        workflow = (TOOLS.parent / '.github/workflows/check.yml').read_text()
        self.assertNotIn('rustup toolchain install 1.', workflow)
        self.assertIn('needs: [reader, capabilities]', workflow)
        self.assertIn('python3 tools/ci.py install', workflow)
        self.assertIn('--compiler "${{ matrix.compiler }}"', workflow)
        for profile in ci.PROFILES:
            if profile != 'default':
                self.assertIn(f'profile: {profile}', workflow)
        self.assertIn('name: lume-linux-x86_64', workflow)
        self.assertIn('contents: read', workflow)


if __name__ == '__main__':
    unittest.main()
