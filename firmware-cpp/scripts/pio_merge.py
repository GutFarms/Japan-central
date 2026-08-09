Import("env")
import hashlib
import os
import shutil
import subprocess
import sys
from pathlib import Path

def after_build(source, target, env):
    build_dir = Path(env.subst("$BUILD_DIR"))
    project = Path(env["PROJECT_DIR"])
    out_dir = project.parent / "flash"
    out_dir.mkdir(parents=True, exist_ok=True)

    bootloader = build_dir / "bootloader.bin"
    partitions = build_dir / "partitions.bin"
    app = build_dir / "firmware.bin"

    boot_app0 = None
    homedir = Path.home() / ".platformio" / "packages"
    for p in sorted(homedir.glob("framework-arduinoespressif32*/tools/partitions/boot_app0.bin")):
        boot_app0 = p
        break

    # Prefer PlatformIO's bundled esptool.py
    esptool_py = None
    for p in sorted(homedir.glob("tool-esptoolpy*/esptool.py")):
        esptool_py = p
        break

    merged = out_dir / "esp32-2432s028-scrypt-miner-merged.bin"
    app_out = out_dir / "esp32-2432s028-scrypt-miner.bin"

    if not app.exists():
        print("merge: app missing, skip")
        return

    shutil.copy2(app, app_out)

    if not bootloader.exists() or not partitions.exists():
        print("merge: bootloader/partitions missing, copied app only")
        return

    if esptool_py is None:
        print("merge: esptool.py not found under ~/.platformio/packages")
        return

    args = [
        env.subst("$PYTHONEXE"),
        str(esptool_py),
        "--chip", "esp32",
        "merge_bin",
        "-o", str(merged),
        "--flash_mode", "dio",
        "--flash_freq", "40m",
        "--flash_size", "4MB",
        "0x1000", str(bootloader),
        "0x8000", str(partitions),
    ]
    if boot_app0 and boot_app0.exists():
        args += ["0xe000", str(boot_app0)]
    args += ["0x10000", str(app)]

    print("merge:", " ".join(args))
    subprocess.check_call(args)

    dl = out_dir / "downloads"
    dl.mkdir(exist_ok=True)

    lines = []
    for name in ("esp32-2432s028-scrypt-miner.bin", "esp32-2432s028-scrypt-miner-merged.bin"):
        p = out_dir / name
        if p.exists():
            h = hashlib.sha256(p.read_bytes()).hexdigest()
            lines.append(f"{h}  {name}")
    (out_dir / "SHA256SUMS.txt").write_text("\n".join(lines) + "\n")
    print("merge: wrote", merged, "size", merged.stat().st_size)

env.AddPostAction("$BUILD_DIR/firmware.bin", after_build)
