#!/usr/bin/env python3
"""Equivalence proof for the `unity-heap` patch, run on the player's own libunity.so.

Emulates (unicorn, arm64) two code paths on a fresh, poisoned object and compares the result:
  (a) the patched UnityDefaultAllocator constructor (bytes from patcher/src/patches.rs),
  (b) the engine's own inline construction of its ALLOC_FALLBACK DynamicHeapAllocator.
They must write byte-identical objects and make the same (stubbed) mutex-init call.

usage: JDR_APK=/path/game.apk python3 tools/proofs/unity_heap_equiv.py
requires: pip install unicorn pyelftools
"""
import io, os, re, struct, sys, zipfile

from elftools.elf.elffile import ELFFile
from unicorn import Uc, UC_ARCH_ARM64, UC_MODE_ARM, UC_HOOK_CODE
from unicorn.arm64_const import UC_ARM64_REG_LR, UC_ARM64_REG_PC, UC_ARM64_REG_SP, UC_ARM64_REG_X0, UC_ARM64_REG_X1, UC_ARM64_REG_X19, UC_ARM64_REG_X20

CTOR = 0x9FF824          # UnityDefaultAllocator ctor (patched)
FALLBACK = (0x50D27C, 0x50D2E0)  # inline DynamicHeapAllocator construction in MemoryManager
MUTEX_INIT = 0xA234C8

apk = os.environ.get("JDR_APK")
if not apk:
    print("skipped: set JDR_APK")
    sys.exit(0)
lib = zipfile.ZipFile(apk).read("lib/arm64-v8a/libunity.so")
src = open(os.path.join(os.path.dirname(__file__), "../../patcher/src/patches.rs")).read()
words = re.search(r"UNITY_HEAP_CTOR: \[u32; 24\] = \[(.*?)\];", src, re.S).group(1)
patch = b"".join(struct.pack("<I", int(w, 16)) for w in re.findall(r"0x[0-9a-f]+", words))
assert len(patch) == 96
elf = ELFFile(io.BytesIO(lib))


def run(start, stop, regs, patched):
    mu = Uc(UC_ARCH_ARM64, UC_MODE_ARM)
    mu.mem_map(0, 0x1600000)
    data = bytearray(lib)
    if patched:
        data[CTOR:CTOR + 96] = patch
    for p in elf.iter_segments():
        if p["p_type"] == "PT_LOAD":
            mu.mem_write(p["p_vaddr"], bytes(data[p["p_offset"]:p["p_offset"] + p["p_filesz"]]))
    obj, stk, name = 0x2000000, 0x3000000, 0x2100000
    mu.mem_map(obj, 0x200000)
    mu.mem_map(stk - 0x10000, 0x20000)
    mu.mem_write(obj, b"\xAA" * 0x200)
    mu.mem_write(name, b"ALLOC_TEST\0")
    calls = []

    def hook(uc, addr, size, _):
        if addr == MUTEX_INIT:
            calls.append(uc.reg_read(UC_ARM64_REG_X0) - obj)
            uc.reg_write(UC_ARM64_REG_PC, uc.reg_read(UC_ARM64_REG_LR))

    mu.hook_add(UC_HOOK_CODE, hook)
    mu.reg_write(UC_ARM64_REG_SP, stk)
    for r, v in regs(obj, name).items():
        mu.reg_write(r, v)
    mu.reg_write(UC_ARM64_REG_LR, stop)
    mu.emu_start(start, stop, timeout=5_000_000)
    return bytes(mu.mem_read(obj, 0x140)), calls


# 1. Who calls the constructor and the page-table code? (must be only the 3 allocator setups and
#    UnityDefaultAllocator's own methods; nothing may keep using the replaced class)
text = elf.get_section_by_name(".text")
base, off, size = text["sh_addr"], text["sh_offset"], text["sh_size"]
words = struct.unpack_from("<%dI" % (size // 4), lib, off)


def callers(target):
    out = []
    for i, w in enumerate(words):
        if (w & 0x7C000000) == 0x14000000:  # B / BL
            imm = w & 0x3FFFFFF
            if imm & (1 << 25):
                imm -= 1 << 26
            if base + i * 4 + imm * 4 == target:
                out.append(base + i * 4)
    return out


ctor_callers = callers(CTOR)
assert ctor_callers == [0x50D704, 0x50D818, 0x50D858], [hex(c) for c in ctor_callers]
assert callers(0x9FFCAC) == [0x9FF968], "page-table register must only be called from UnityDefaultAllocator::Allocate"
assert callers(0x9FFF04) == [0x9FFB24], "page-table unregister must only be called from UnityDefaultAllocator::Deallocate"
print("callers: constructor only from ALLOC_DEFAULT/ALLOC_GAMEOBJECT/ALLOC_GFX setup; page table only from its own class")

# 2. Each of those three objects gets 0x140 bytes of storage (bump `add x9, x20|x21, #0x140`
#    right before each call); the new constructor writes up to offset 0x118.
for c in ctor_callers:
    window = struct.unpack_from("<16I", lib, c - 0x40)
    assert any((w & 0xFFFFFC00) == (0x91000000 | (0x140 << 10)) for w in window), hex(c)
print("storage: 0x140 bytes reserved per allocator, constructor writes < 0x118")

a, ca = run(CTOR, 0x10, lambda o, n: {UC_ARM64_REG_X0: o, UC_ARM64_REG_X1: n}, True)
b, cb = run(FALLBACK[0], FALLBACK[1], lambda o, n: {UC_ARM64_REG_X20: o, UC_ARM64_REG_X1: n, UC_ARM64_REG_X19: 0x2180000}, False)
diff = [i for i in range(0x140) if a[i] != b[i]]
written = sum(1 for x in a if x != 0xAA)
print(f"patched ctor wrote {written} bytes; mutex init at {ca} vs {cb}; differing bytes: {len(diff)}")
if diff or ca != cb:
    print("FAIL")
    sys.exit(1)
print("OK: patched constructor builds the same DynamicHeapAllocator as the engine's ALLOC_FALLBACK")
