"""Source-derived compiler selection and bounded capability qualification."""

import argparse
import hashlib
import os
from pathlib import Path
import re
import subprocess
import tomllib


ROOT = Path(__file__).resolve().parents[1]
PROFILES = {
    'default': (),
    'minimal': ('--no-default-features',),
    'text': ('--no-default-features', '--features', 'math,syntax,miniflux'),
    'graphics': ('--no-default-features', '--features', 'graphic-math,sixel'),
    'svg': ('--no-default-features', '--features', 'svg'),
    'sixel': ('--no-default-features', '--features', 'sixel'),
}


def compiler_configuration(root, compiler):
    """Keep release version/components and minimum version at their owners."""
    if compiler == 'pinned':
        configuration = tomllib.loads(
            (root / 'rust-toolchain.toml').read_text(),
        )['toolchain']
        version = configuration['channel']
        if not re.fullmatch(r'\d+\.\d+\.\d+', version):
            raise ValueError('Release toolchain must be an exact Rust version')
        return (version, configuration.get('profile', 'default'),
                configuration.get('components', []))
    manifest = tomllib.loads((root / 'Cargo.toml').read_text())
    version = manifest['package']['rust-version']
    if not re.fullmatch(r'\d+\.\d+(?:\.\d+)?', version):
        raise ValueError('Minimum Rust version must be numeric')
    if version.count('.') == 1:
        version += '.0'
    return version, 'minimal', []


def install_command(configuration):
    version, profile, components = configuration
    command = ['rustup', 'toolchain', 'install', version, '--profile', profile]
    for component in components:
        command.extend(['--component', component])
    return command


def qualification_commands(version, compiler, profile, offline=False):
    cargo = ['cargo', f'+{version}']
    arguments = ['--locked', '--all-targets', *PROFILES[profile]]
    if offline:
        arguments.append('--offline')
    commands = []
    if compiler == 'pinned':
        commands.append([*cargo, 'clippy', *arguments, '--', '-D', 'warnings'])
    commands.append([*cargo, 'test', *arguments, '--', '--test'])
    return commands


def install(configuration, root):
    subprocess.run(install_command(configuration), check=True, cwd=root)
    version = configuration[0]
    identity = subprocess.run(
        ['rustc', f'+{version}', '-Vv'], check=True, cwd=root,
        capture_output=True, text=True,
    ).stdout
    print(identity, end='')
    fingerprint = hashlib.sha256(identity.encode()).hexdigest()
    output = os.environ.get('GITHUB_OUTPUT')
    if output:
        with Path(output).open('a') as stream:
            stream.write(f'fingerprint={fingerprint}\nversion={version}\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('install', 'qualify'))
    parser.add_argument('--compiler', choices=('pinned', 'minimum'),
                        default='pinned')
    parser.add_argument('--profile', choices=PROFILES, default='default')
    parser.add_argument('--offline', action='store_true')
    arguments = parser.parse_args()
    try:
        configuration = compiler_configuration(ROOT, arguments.compiler)
        if arguments.action == 'install':
            install(configuration, ROOT)
        else:
            commands = qualification_commands(
                configuration[0], arguments.compiler, arguments.profile,
                arguments.offline,
            )
            for command in commands:
                print(' '.join(command), flush=True)
                subprocess.run(command, check=True, cwd=ROOT)
    except (OSError, ValueError, KeyError,
            subprocess.CalledProcessError) as error:
        parser.exit(1, f'Compiler qualification failed: {error}\n')


if __name__ == '__main__':
    main()
