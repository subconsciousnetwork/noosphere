#!/usr/bin/env python3
import shutil
import sys
import argparse
import os
import subprocess
import ns_utils
import logging

ROOT_DIR = os.path.realpath(os.path.join(os.path.dirname(os.path.realpath(__file__)), '..'))

INSTALL_DIRECTIVES = {
    'linux': {
        'pm': 'apt-get',
        'setup': 'sudo apt-get update -qqy',
        'install': 'sudo apt-get install $0',
        'packages': ['jq', 'protobuf-compiler', 'cmake'],
    },
    'darwin': {
        'pm': 'brew',
        'setup': 'brew update',
        'install': 'brew install $0',
        'packages': ['protobuf', 'cmake'],
    },
    'win32': {
        'pm': 'choco',
        'setup': '',
        'install': 'choco install -y $0',
        'packages': ['cmake', 'protoc', 'openssl']
    },
}

def error_quit(message: str):
    logging.error(message)
    sys.exit(1)

def run_command(command: str, cwd=None):
    subprocess.run(command.split(' '), cwd=cwd)

def build_command(buildtype: str, is_release: bool, is_verbose: bool):
    if buildtype == 'rust':
        print('build')

# `noodev install`: Installs system dependencies.
def install_command(is_verbose: bool):
    directive = INSTALL_DIRECTIVES[sys.platform]
    if directive == None:
        error_quit(f'Unsupported platform: {sys.platform}')

    if shutil.which(directive['pm']) == None:
        error_quit(f'Could not find {directive["pm"]} installed in path.')
    
    if shutil.which('rustup') == None:
        logging.info('Installing rustup...')
        run_command('curl -sSf https://sh.rustup.rs | sh -s -- -y') 
    
    logging.info('Updating rustup...')
    run_command('rustup update')

    logging.info('Installing rustup components...')
    run_command('rustup component add clippy rustfmt')

    if len(directive['setup']) > 0:
        logging.info('Updating package manager...')
        run_command(directive['setup'], ROOT_DIR)

    install_command = directive['install'].replace('$0', ' '.join(directive['packages']))
    logging.info('Installing packages...')
    run_command(install_command, ROOT_DIR)

    logging.warning('`install` command not implemented.')
    print(directive)

def docs_command(is_verbose: bool):
    logging.warning('`docs` command not implemented.')
    sys.exit(1)

def test_command(is_verbose: bool):
    logging.warning('`test` command not implemented.')
    sys.exit(1)

def main():
    parser = argparse.ArgumentParser(
                    prog='noodev',
                    description='Commands for developing Noosphere')
    subparsers = parser.add_subparsers(dest='subcommand', help='Subcommands.')
    parser.add_argument('-v', action='store_true',
        help='Enable verbose output.')

    # `noodev build`
    build_parser = subparsers.add_parser('build',
        help='Build a Noosphere component.') 
    build_parser.add_argument('buildtype', choices=['rust', 'swift'],
        help='Noosphere component to build.')
    build_parser.add_argument('--release', action='store_true',
        help='Build with release configuration.')

    # `noodev install`
    install_parser = subparsers.add_parser('install',
        help='Install system dependencies for working on Noosphere.')
    # `noodev test`
    test_parser = subparsers.add_parser('test',
        help='Run Noosphere tests.')
    # `noodev docs`
    docs_parser = subparsers.add_parser('docs',
        help='Generate Noosphere documentation.')

    args = parser.parse_args()
    print(args)

    if args.subcommand == 'build':
        build_command(args.buildtype, args.release, args.v)
    elif args.subcommand == 'install':
        install_command(args.v)
    elif args.subcommand == 'test':
        test_command(args.v)
    elif args.subcommand == 'docs':
        docs_command(args.v)
    else:
        logging.error(f'Invalid command: {args.subcommand}')
        sys.exit(1)


if __name__ == "__main__":
    main()
  
