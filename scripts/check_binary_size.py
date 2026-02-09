#!/usr/bin/env python3
"""Check firmware ELF size against app partition limits."""

import csv
import subprocess
import sys
from pathlib import Path


def parse_size(size_str: str) -> int:
    size_str = size_str.strip()
    if size_str.startswith(("0x", "0X")):
        return int(size_str, 16)
    return int(size_str)


def get_partition_size(partitions_csv: Path, app_label: str) -> int:
    """Extract selected app partition size from partitions.csv."""
    with open(partitions_csv) as f:
        reader = csv.reader(f)
        for row in reader:
            # Skip comments and empty lines
            if not row or row[0].startswith("#") or not row[0].strip():
                continue

            name, ptype, subtype, offset, size = row[:5]
            name = name.strip()
            ptype = ptype.strip()

            # Match by explicit label first.
            if name == app_label:
                return parse_size(size)
            # Backward-compatible fallback if caller asks for logical app0.
            if app_label == "app0" and ptype == "app" and subtype.strip() in {"ota_0", "factory"}:
                return parse_size(size)

    raise ValueError(
        f"Partition '{app_label}' not found in {partitions_csv}"
    )


def get_binary_size(elf_path: Path) -> int:
    """Get binary size from ELF file using cargo size or stat."""
    # First try to get the size using llvm-size if available
    try:
        result = subprocess.run(
            ["riscv32-esp-elf-size", "-A", str(elf_path)],
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            # Parse the total size from output
            for line in result.stdout.split("\n"):
                if "Total" in line:
                    return int(line.split()[-1])
    except FileNotFoundError:
        pass

    # Fallback: use stat to get file size
    return elf_path.stat().st_size


def format_bytes(size: int) -> str:
    """Format bytes in human readable format."""
    if size < 1024:
        return f"{size}B"
    elif size < 1024 * 1024:
        return f"{size / 1024:.1f}KB"
    else:
        return f"{size / (1024 * 1024):.2f}MB"


def main():
    if len(sys.argv) != 4:
        print(
            "Usage: check_binary_size.py <partitions_csv> <elf_path> <app_partition_label>",
            file=sys.stderr,
        )
        sys.exit(2)

    repo_root = Path(__file__).parent.parent
    partitions_csv = (repo_root / sys.argv[1]).resolve()
    elf_path = (repo_root / sys.argv[2]).resolve()
    app_partition_label = sys.argv[3]

    if not partitions_csv.exists():
        print(f"❌ Error: Partition table not found: {partitions_csv}")
        sys.exit(1)

    if not elf_path.exists():
        print("❌ Error: Firmware binary not found.")
        print(f"   Expected at: {elf_path}")
        print("   Run 'just build-firmware' first.")
        sys.exit(1)

    # Get sizes
    partition_size = get_partition_size(partitions_csv, app_partition_label)
    binary_size = get_binary_size(elf_path)

    # Check
    percentage = (binary_size / partition_size) * 100
    remaining = partition_size - binary_size

    print(f"📊 Firmware Size Check")
    print(f"   Binary size:      {format_bytes(binary_size)} ({binary_size} bytes)")
    print(
        f"   App partition:    {format_bytes(partition_size)} ({partition_size} bytes)"
    )
    print(f"   Usage:            {percentage:.1f}%")
    print(f"   Remaining space:  {format_bytes(remaining)}")

    if binary_size > partition_size:
        print(
            f"\n❌ Error: Binary is {format_bytes(binary_size - partition_size)} too large!"
        )
        print(f"   Partition: {partitions_csv}")
        print(f"   Binary:    {elf_path}")
        print(f"\n   Suggestions:")
        print(f"   - Increase app partition size in {partitions_csv}")
        print(f"   - Enable size optimizations in Cargo.toml")
        print(f"   - Remove unused features or dependencies")
        sys.exit(1)
    elif percentage > 90:
        print(f"\n⚠️  Warning: Binary is at {percentage:.1f}% of partition limit!")
        print(f"   Consider increasing partition size or optimizing binary size.")
        sys.exit(0)
    else:
        print(f"\n✅ Binary fits within partition ({percentage:.1f}% used)")
        sys.exit(0)


if __name__ == "__main__":
    main()
