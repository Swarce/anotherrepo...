#!/usr/bin/env python3
"""Check a generated `versionfile` bundle with UnityPy (an independent Unity reader).

The bundle must parse, expose the container path the client loads, point its MonoScript at
ScSystem.ScIO.AssetBundleVersionInfo, and decode every manifest entry through our type tree.

usage: python3 tools/proofs/versionfile_check.py <versionfile.bytes> [expected_entries]
requires: pip install UnityPy
"""
import sys

import UnityPy

path = sys.argv[1]
expected = int(sys.argv[2]) if len(sys.argv) > 2 else None
env = UnityPy.load(open(path, "rb").read())
container = "assets/resources/version/android/versionfile.asset"
assert list(env.container.keys()) == [container], list(env.container.keys())
kinds = {}
for o in env.objects:
    kinds[o.type.name] = o.read_typetree()
ab, ms, mb = kinds["AssetBundle"], kinds["MonoScript"], kinds["MonoBehaviour"]
assert ab["m_AssetBundleName"] == "versionfile" and ab["m_Container"][0][1]["preloadSize"] == 2
assert (ms["m_Namespace"], ms["m_ClassName"], ms["m_AssemblyName"]) == ("ScSystem.ScIO", "AssetBundleVersionInfo", "Assembly-CSharp.dll")
entries = mb["VersionData"]
assert all(e["isObfuscation"] and e["path"].endswith(".bytes") and e["ObfuscationPath"].endswith(".bytes") and len(e["md5Sum"]) == 32 and e["size"] > 0 for e in entries)
assert mb["TotalSize"] == sum(e["size"] for e in entries)
if expected is not None:
    assert len(entries) == expected, (len(entries), expected)
print(f"OK: versionfile parses with UnityPy; {len(entries)} entries ({sum(e['isEncrypt'] for e in entries)} encrypted)")
