#!/usr/bin/env python3
"""
Rust Enum Generator

Generates Rust enum code with feature flags and AsRef<[u8]> implementation
from binary asset files in a directory.
"""

import os
import re
import argparse
from pathlib import Path
from typing import List, Tuple, Dict


def parse_filename(filename: str) -> Tuple[str, str]:
    """
    Parse filename to extract feature name and variant name.
    
    Expected format: irc_1002_c.bin -> (irc1002, Irc1002)
    """
    # Remove extension
    base = Path(filename).stem[:-1]
    
    # Convert to feature name (lowercase, no underscores)
    feature_name = re.sub(r'[_\s]+', '', base.lower())
    
    # Convert to variant name (PascalCase)
    # Split on underscores, capitalize each part, join
    parts = base.split('_')
    variant_name = ''.join(word.capitalize() for word in parts)
    
    return feature_name, variant_name


def scan_assets(directory: str) -> List[Tuple[str, str, str]]:
    """
    Scan directory for asset files and return (filename, feature, variant) tuples.
    """
    assets = []
    
    if not os.path.exists(directory):
        raise FileNotFoundError(f"Directory '{directory}' not found")
    
    for filename in os.listdir(directory):
        if filename.endswith('.bin'):
            feature, variant = parse_filename(filename)
            assets.append((filename, feature, variant))
    
    assets.sort(key=lambda x: x[1])  # Sort by feature name
    return assets


def generate_features_list(assets: List[Tuple[str, str, str]]) -> str:
    """Generate list of feature names."""

    proc = lambda x : f'  "{x}",'
    in_table = '\n'.join([proc(asset[1]) for asset in assets])
    all = f'all_subjects = [\n{in_table}\n]\n'

    proc = lambda x : f'{x} = []'
    features = [proc(asset[1]) for asset in assets]
    base = '\n'.join(features)

    return '\n'.join([all, base])


def generate_enum(assets: List[Tuple[str, str, str]]) -> str:
    """Generate the Rust enum with feature flags."""
    lines = ["pub enum Subject {"]
    
    for filename, feature, variant in assets:
        lines.append(f'    #[cfg(feature = "{feature}")]')
        lines.append(f'    {variant},')
    
    lines.append("}")
    return '\n'.join(lines)


def generate_asref_impl(assets: List[Tuple[str, str, str]]) -> str:
    """Generate the AsRef<[u8]> implementation."""
    lines = [
        "impl AsRef<[u8]> for Subject {",
        "    fn as_ref(&self) -> &[u8] {",
        "        match self {"
    ]
    
    for filename, feature, variant in assets:
        filename = f'../assets/{filename}'
        lines.append(f'            #[cfg(feature = "{feature}")]')
        lines.append(f'            Subject::{variant} => include_bytes!("{filename}"),')
    
    lines.extend([
        "        }",
        "    }",
        "}"
    ])
    
    return '\n'.join(lines)


def generate_full_rust_code(assets: List[Tuple[str, str, str]]) -> str:
    """Generate complete Rust code file."""
    enum_code = generate_enum(assets)
    asref_impl = generate_asref_impl(assets)
    
    return f"""
{enum_code}

{asref_impl}
"""


def main():
    parser = argparse.ArgumentParser(
        description="Generate Rust enum code from binary asset files"
    )
    parser.add_argument(
        "directory",
        help="Directory containing .bin asset files"
    )
    parser.add_argument(
        "--output", "-o",
        choices=["enum", "features", "full"],
        default="full",
        help="What to output: 'enum' (just the enum), 'features' (feature list), or 'full' (complete Rust code)"
    )
    
    args = parser.parse_args()
    
    try:
        assets = scan_assets(args.directory)
        
        if not assets:
            print(f"No .bin files found in '{args.directory}'")
            return
        
        if args.output == "features":
            print(generate_features_list(assets))
        elif args.output == "enum":
            print(generate_enum(assets))
        else:  # full
            print(generate_full_rust_code(assets))
            
    except FileNotFoundError as e:
        print(f"Error: {e}")
    except Exception as e:
        print(f"Unexpected error: {e}")


if __name__ == "__main__":
    main()
