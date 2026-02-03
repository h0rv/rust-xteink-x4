import argparse
import os
import sys
import time
import binascii

import serial


def open_serial(port: str, baud: int, timeout: float) -> serial.Serial:
    return serial.Serial(port=port, baudrate=baud, timeout=timeout)


def write_line(ser: serial.Serial, line: str) -> None:
    ser.write((line + "\r\n").encode())


def read_line(ser: serial.Serial, timeout: float) -> str:
    deadline = time.time() + timeout
    while time.time() < deadline:
        raw = ser.readline()
        if not raw:
            continue
        line = raw.decode(errors="replace").strip()
        if line:
            return line
    raise TimeoutError("Timed out waiting for response")


def read_response(ser: serial.Serial, timeout: float) -> list[str]:
    lines: list[str] = []
    while True:
        line = read_line(ser, timeout)
        if line.startswith("OK"):
            return lines
        if line.startswith("ERR"):
            raise RuntimeError(line)
        lines.append(line)


def cmd_ls(ser: serial.Serial, path: str, timeout: float) -> None:
    write_line(ser, f"ls {path}")
    for line in read_response(ser, timeout):
        print(line)


def cmd_rm(ser: serial.Serial, path: str, timeout: float) -> None:
    write_line(ser, f"rm {path}")
    read_response(ser, timeout)


def cmd_rmdir(ser: serial.Serial, path: str, timeout: float) -> None:
    write_line(ser, f"rmdir {path}")
    read_response(ser, timeout)


def cmd_mkdir(ser: serial.Serial, path: str, timeout: float) -> None:
    write_line(ser, f"mkdir {path}")
    read_response(ser, timeout)


def cmd_cat(ser: serial.Serial, path: str, timeout: float) -> None:
    write_line(ser, f"cat {path}")
    for line in read_response(ser, timeout):
        print(line)


def cmd_refresh(ser: serial.Serial, mode: str, timeout: float) -> None:
    write_line(ser, f"refresh {mode}")
    read_response(ser, timeout)


def cmd_exists(ser: serial.Serial, path: str, timeout: float) -> None:
    write_line(ser, f"exists {path}")
    lines = read_response(ser, timeout)
    if lines:
        print(lines[0])
    else:
        print("0")


def cmd_stat(ser: serial.Serial, path: str, timeout: float) -> tuple[str, int] | None:
    write_line(ser, f"stat {path}")
    lines = read_response(ser, timeout)
    if not lines:
        return None
    parts = lines[0].split()
    if len(parts) != 2:
        return None
    kind = parts[0]
    size = int(parts[1])
    print(f"{kind} {size}")
    return kind, size


def cmd_sleep(ser: serial.Serial, timeout: float) -> None:
    write_line(ser, "sleep")
    read_response(ser, timeout)


def cmd_put(
    ser: serial.Serial, local_path: str, remote_path: str, timeout: float
) -> None:
    size = os.path.getsize(local_path)
    chunk_size = 1024
    write_line(ser, f"put {remote_path} {size} {chunk_size}")
    done_timeout = max(timeout, size / 2000.0 + 8.0)
    line = read_line(ser, done_timeout)
    if not line.startswith("OK READY"):
        raise RuntimeError(line)

    crc = 0
    bytes_sent = 0
    last_reported = 0
    with open(local_path, "rb") as handle:
        while True:
            chunk = handle.read(chunk_size)
            if not chunk:
                break
            crc = binascii.crc32(chunk, crc)
            ser.write(chunk)
            bytes_sent += len(chunk)
            ack = read_line(ser, done_timeout)
            if not ack.startswith("OK "):
                raise RuntimeError(ack)
            if bytes_sent - last_reported >= 4096 or bytes_sent == size:
                percent = (bytes_sent / size * 100.0) if size else 100.0
                sys.stdout.write(f"\rUpload {bytes_sent}/{size} bytes ({percent:.1f}%)")
                sys.stdout.flush()
                last_reported = bytes_sent

    ser.flush()

    if size:
        sys.stdout.write("\n")
        sys.stdout.flush()

    line = read_line(ser, done_timeout)
    if not line.startswith("OK DONE"):
        raise RuntimeError(line)
    parts = line.split()
    if len(parts) >= 3:
        remote_crc = parts[2]
        if remote_crc.strip() != f"{crc & 0xFFFFFFFF:08x}":
            raise RuntimeError(
                f"CRC mismatch: device={remote_crc} local={crc & 0xFFFFFFFF:08x}"
            )


def main() -> int:
    parser = argparse.ArgumentParser(description="Xteink X4 serial CLI")
    parser.add_argument("--port", default="/dev/ttyUSB0")
    parser.add_argument("--baud", default=115200, type=int)
    parser.add_argument("--timeout", default=2.0, type=float)

    sub = parser.add_subparsers(dest="cmd", required=True)

    ls_cmd = sub.add_parser("ls")
    ls_cmd.add_argument("path", nargs="?", default="/")

    rm_cmd = sub.add_parser("rm")
    rm_cmd.add_argument("path")

    rmdir_cmd = sub.add_parser("rmdir")
    rmdir_cmd.add_argument("path")

    mkdir_cmd = sub.add_parser("mkdir")
    mkdir_cmd.add_argument("path")

    cat_cmd = sub.add_parser("cat")
    cat_cmd.add_argument("path")

    put_cmd = sub.add_parser("put")
    put_cmd.add_argument("local")
    put_cmd.add_argument("remote")

    refresh_cmd = sub.add_parser("refresh")
    refresh_cmd.add_argument(
        "mode", choices=["fast", "partial", "full"], default="fast"
    )

    exists_cmd = sub.add_parser("exists")
    exists_cmd.add_argument("path")

    stat_cmd = sub.add_parser("stat")
    stat_cmd.add_argument("path")

    sub.add_parser("sleep")
    sub.add_parser("help")

    args = parser.parse_args()

    try:
        with open_serial(args.port, args.baud, args.timeout) as ser:
            if args.cmd == "ls":
                cmd_ls(ser, args.path, args.timeout)
            elif args.cmd == "rm":
                cmd_rm(ser, args.path, args.timeout)
            elif args.cmd == "rmdir":
                cmd_rmdir(ser, args.path, args.timeout)
            elif args.cmd == "mkdir":
                cmd_mkdir(ser, args.path, args.timeout)
            elif args.cmd == "cat":
                cmd_cat(ser, args.path, args.timeout)
            elif args.cmd == "put":
                cmd_put(ser, args.local, args.remote, args.timeout)
            elif args.cmd == "refresh":
                cmd_refresh(ser, args.mode, args.timeout)
            elif args.cmd == "exists":
                cmd_exists(ser, args.path, args.timeout)
            elif args.cmd == "stat":
                cmd_stat(ser, args.path, args.timeout)
            elif args.cmd == "sleep":
                cmd_sleep(ser, args.timeout)
            elif args.cmd == "help":
                write_line(ser, "help")
                for line in read_response(ser, args.timeout):
                    print(line)
    except Exception as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
