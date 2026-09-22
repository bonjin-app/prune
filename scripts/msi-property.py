#!/usr/bin/env python3
"""Reads properties out of a Windows Installer package, on any platform.

    scripts/msi-property.py Prune_0.1.3_x64_en-US.msi ProductCode

Used at release time to put the real ProductCode into the WinGet manifest. Tauri's WiX
template generates a fresh one for every build, so it changes with each release, and a
manifest carrying the previous one is worse than one carrying none: WinGet would match an
installed copy against a code that no longer exists and get upgrade and uninstall wrong.

An MSI is an OLE compound document whose stream names are encoded in a private Unicode range,
and whose tables are arrays of indices into a shared string pool. Both are decoded here.
"""
import struct
import sys

try:
    import olefile
except ModuleNotFoundError:
    sys.exit("olefile is needed to read an MSI: pip3 install --user olefile")

MIME = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz._"


def decode_name(name: str) -> str:
    """MSI stream names pack two base64 characters into one code point."""
    out = []
    for ch in name:
        c = ord(ch)
        if 0x3800 <= c < 0x4800:
            c -= 0x3800
            out.append(MIME[c & 0x3F])
            out.append(MIME[(c >> 6) & 0x3F])
        elif 0x4800 <= c < 0x4840:
            out.append(MIME[c - 0x4800])
        else:
            out.append(ch)
    return "".join(out)


def properties(path: str) -> dict[str, str]:
    ole = olefile.OleFileIO(path)
    streams = {decode_name("/".join(e)).lstrip("䡀"): e for e in ole.listdir()}

    pool = ole.openstream(streams["_StringPool"]).read()
    data = ole.openstream(streams["_StringData"]).read()
    codepage = struct.unpack_from("<H", pool, 0)[0]
    encoding = "utf-8" if codepage in (0, 65001) else "cp1252"

    strings = [""]  # the pool is 1-based; index 0 is "no string"
    offset, i = 0, 4
    while i + 4 <= len(pool):
        length, refs = struct.unpack_from("<HH", pool, i)
        i += 4
        if length == 0 and refs != 0:
            # A string longer than 64KB carries its high bits in the following entry.
            high, _ = struct.unpack_from("<HH", pool, i)
            i += 4
            length = (refs << 16) | high
        strings.append(data[offset:offset + length].decode(encoding, "replace"))
        offset += length

    table = ole.openstream(streams["Property"]).read()
    width = 2 if len(strings) < 0x10000 else 4
    fmt = "<H" if width == 2 else "<I"
    rows = len(table) // (width * 2)
    # Columns are stored one after another, not row by row: every name, then every value.
    names = [struct.unpack_from(fmt, table, k * width)[0] for k in range(rows)]
    values = [struct.unpack_from(fmt, table, (rows + k) * width)[0] for k in range(rows)]
    return {
        strings[n]: strings[v]
        for n, v in zip(names, values)
        if n < len(strings) and v < len(strings)
    }


def main() -> None:
    if len(sys.argv) not in (2, 3):
        sys.exit(f"usage: {sys.argv[0]} <file.msi> [PropertyName]")
    found = properties(sys.argv[1])
    if len(sys.argv) == 2:
        for key in sorted(found):
            print(f"{key}={found[key]}")
        return
    wanted = sys.argv[2]
    if wanted not in found:
        sys.exit(f"{wanted} is not set in {sys.argv[1]}")
    print(found[wanted])


if __name__ == "__main__":
    main()
