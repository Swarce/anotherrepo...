#!/usr/bin/env python3
"""Check a generated `in_memory_master_data` bundle with UnityPy (an independent Unity reader).

Every table must sit at the lowercase container path the client asks for, point at a MonoScript
with the right namespace/class, and decode through our type tree consuming exactly its bytes.
The values read back must equal our content files (server/content/masterdata/*.json).

usage: python3 tools/proofs/masterdata_check.py <bundle> <repo>/server/content/masterdata
       (generated tables are read from <bundle>.generated/ when present)
requires: pip install UnityPy
"""
import json
import os
import sys

import UnityPy

bundle, content = sys.argv[1], sys.argv[2]
schema = json.load(open(os.path.join(content, "schema.json")))
files = {f[:-5]: json.load(open(os.path.join(content, f))) for f in os.listdir(content) if f.endswith(".json") and f != "schema.json"}
# Tables the server generates from other content (written next to the bundle by the real test).
gen_dir = bundle + ".generated"
if os.path.isdir(gen_dir):
    files.update({f[:-5]: json.load(open(os.path.join(gen_dir, f))) for f in os.listdir(gen_dir) if f.endswith(".json")})
want = [a for a in schema["assets"] if a["core"] or a["name"] in files]
prefix = "assets/resources/common/inmemorymasterdata/"

env = UnityPy.load(open(bundle, "rb").read())
keys = sorted(env.container.keys())
assert keys == sorted(prefix + a["name"].lower() + ".asset" for a in want), keys

ab = next(o for o in env.objects if o.type.name == "AssetBundle").read_typetree()
assert ab["m_AssetBundleName"] == "in_memory_master_data"
assert [c[0] for c in ab["m_Container"]] == sorted(c[0] for c in ab["m_Container"]), "container sorted"

scripts = {o.path_id: o.read_typetree() for o in env.objects if o.type.name == "MonoScript"}


import struct

SEC = "ScSystem.ScSecurity."


def default(tname, is_list):
    if is_list:
        return []
    t = schema["types"].get(tname)
    if tname in ("int", "long") or (t and t["kind"] == "enum"):
        return 0
    return {"float": 0.0, "bool": False, "string": "", "Vector3": [0, 0, 0], "Quaternion": [0, 0, 0, 1]}.get(tname) \
        if t is None else {f["name"]: default(f["type"], f.get("list", False)) for f in t["fields"]}


def compare(tname, is_list, src, got, at):
    """src: our JSON value (absent fields = schema defaults); got: what UnityPy read."""
    if is_list:
        assert len(src) == len(got), (at, len(src), len(got))
        for i, (s, g) in enumerate(zip(src, got)):
            compare(tname, False, s, g, f"{at}[{i}]")
        return
    t = schema["types"].get(tname)
    # Shorthands the server accepts: SecurityInt/FloatValue as a plain number (seed 0).
    if tname == SEC + "SecurityIntValue" and isinstance(src, (int, float)):
        assert got["seed"] ^ got["securityValue"] == src, (at, got, src)
        return
    if tname == SEC + "SecurityFloatValue" and isinstance(src, (int, float)):
        bits = (got["seed"] ^ got["securityValue"]) & 0xFFFFFFFF
        assert struct.unpack("<f", struct.pack("<I", bits))[0] == struct.unpack("<f", struct.pack("<f", src))[0], at
        return
    if tname in ("Vector3", "Quaternion"):
        keys = "xyz" if tname == "Vector3" else "xyzw"
        vals = [src[k] for k in keys] if isinstance(src, dict) else src
        assert all(abs(got[k] - v) < 1e-6 for k, v in zip(keys, vals)), (at, got, src)
        return
    if t is None or t["kind"] == "enum":
        exp = t["values"][src] if (t and isinstance(src, str)) else src
        if tname == "bool":
            assert bool(got) == exp, (at, got, exp)
        elif tname == "float":
            assert abs(got - exp) < 1e-6, (at, got, exp)
        else:
            assert got == exp, (at, got, exp)
        return
    for f in t["fields"]:
        ft, fl = f["type"], f.get("list", False)
        compare(ft, fl, src.get(f["name"], default(ft, fl)), got[f["name"]], f"{at}.{f['name']}")


checked = 0
for o in env.objects:
    if o.type.name != "MonoBehaviour":
        continue
    # check_read: UnityPy raises unless the type tree consumes exactly the object's bytes.
    tree = o.read_typetree(check_read=True)
    name = tree["m_Name"]
    a = next(x for x in want if x["name"] == name)
    ns, cls = a["class"].rsplit(".", 1)
    ms = scripts[tree["m_Script"]["m_PathID"]]
    assert (ms["m_Namespace"], ms["m_ClassName"], ms["m_AssemblyName"]) == (ns, cls, "Assembly-CSharp.dll"), (name, ms)
    # Every field: content values, or the schema default where the content (or no file) omits it.
    compare(a["class"], False, files.get(name, {}).get("data", {}), tree, name)
    checked += 1

assert checked == len(want), (checked, len(want))
rows = {n: len(next(iter(v["data"].values()))) for n, v in files.items()}
print(f"OK: in_memory_master_data parses with UnityPy; {checked} tables; content rows {rows}")
