#!/usr/bin/env python3
"""Packs the cross-built Windows launcher: geffeniuse.exe plus the WebView2 loader it links dynamically.
Usage: scripts/package-windows.py <version> [outdir]   (default outdir: ~/ssd)"""
import glob, hashlib, os, sys, zipfile

version = sys.argv[1]
out_dir = os.path.expanduser(sys.argv[2] if len(sys.argv) > 2 else "~/ssd")
root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
exe = os.path.join(root, "target", "x86_64-pc-windows-gnu", "release", "geffeniuse.exe")
dlls = glob.glob(os.path.expanduser("~/.cargo/registry/src/*/webview2-com-sys-*/x64/WebView2Loader.dll"))
assert os.path.isfile(exe), f"missing {exe}"
assert dlls, "WebView2Loader.dll not found in the cargo registry"
dll = sorted(dlls)[-1]
out = os.path.join(out_dir, f"Geffeniuse-Launcher-{version}-windows-x64.zip")
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
    z.write(exe, "geffeniuse.exe")
    z.write(dll, "WebView2Loader.dll")
    z.writestr("README.txt", "Geffeniuse Launcher\r\n\r\nKeep geffeniuse.exe and WebView2Loader.dll in the same folder and start geffeniuse.exe.\r\nNeeds the Microsoft WebView2 runtime (included in Windows 10/11).\r\nUpdates: Options > Updates.\r\n")
sha = hashlib.sha256(open(out, "rb").read()).hexdigest()
print(out, os.path.getsize(out), sha)
