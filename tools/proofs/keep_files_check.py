#!/usr/bin/env python3
"""Check for the `keep-files` patch, on the player's own libil2cpp.so.

SaveDataManager.LoadNonObfuscatedClearFlag (0xFD1534) must be called only by the cache-clear
routine (ResourceDownloadUIController.<OnCacheClear>, call site 0xFD13AC), so making it return
true changes nothing but "skip the one-time wipe of the download folder".

usage: JDR_APK=/path/game.apk python3 tools/proofs/keep_files_check.py
"""
import io, os, struct, sys, zipfile

from elftools.elf.elffile import ELFFile

apk = os.environ.get("JDR_APK")
if not apk:
    print("skipped: set JDR_APK")
    sys.exit(0)
lib = zipfile.ZipFile(apk).read("lib/arm64-v8a/libil2cpp.so")
elf = ELFFile(io.BytesIO(lib))
TARGET, EXPECTED = 0xFD1534, [0xFD13AC]
found = []
for name in (".text", "il2cpp"):
    sec = elf.get_section_by_name(name)
    if sec is None:
        continue
    base, off, size = sec["sh_addr"], sec["sh_offset"], sec["sh_size"]
    for i, w in enumerate(struct.unpack_from("<%dI" % (size // 4), lib, off)):
        if (w & 0x7C000000) == 0x14000000:  # B / BL
            imm = w & 0x3FFFFFF
            if imm & (1 << 25):
                imm -= 1 << 26
            if base + i * 4 + imm * 4 == TARGET:
                found.append(base + i * 4)
if found != EXPECTED:
    print("FAIL: callers", [hex(x) for x in found])
    sys.exit(1)
print("OK: LoadNonObfuscatedClearFlag is only called by the download-folder cache clear")
