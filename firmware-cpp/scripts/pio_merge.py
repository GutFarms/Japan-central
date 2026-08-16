Import("env")
import hashlib
import os
import shutil
import subprocess
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

    esptool_py = None
    for p in sorted(homedir.glob("tool-esptoolpy*/esptool.py")):
        esptool_py = p
        break

    pioenv = env.get("PIOENV", "cyd")
    d0 = pioenv == "cyd-d0" or "CYD_D0_BUILD" in env.subst("$CPPDEFINES")
    # Always emit canonical names; D0 build also emits *-d0.bin aliases.
    names = [
        ("esp32-2432s028-sha256-miner.bin", "esp32-2432s028-sha256-miner-merged.bin"),
    ]
    if d0:
        names.append(
            ("esp32-2432s028-sha256-miner-d0.bin", "esp32-2432s028-sha256-miner-d0-merged.bin")
        )

    if not app.exists():
        print("merge: app missing, skip")
        return

    if not bootloader.exists() or not partitions.exists():
        for app_name, _ in names:
            shutil.copy2(app, out_dir / app_name)
        print("merge: bootloader/partitions missing, copied app only")
        return

    if esptool_py is None:
        print("merge: esptool.py not found under ~/.platformio/packages")
        return

    # Build merged once, then copy to all requested names.
    tmp_merged = out_dir / "_tmp-merged.bin"
    args = [
        env.subst("$PYTHONEXE"),
        str(esptool_py),
        "--chip", "esp32",
        "merge_bin",
        "-o", str(tmp_merged),
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

    lines = []
    for app_name, merged_name in names:
        app_out = out_dir / app_name
        merged_out = out_dir / merged_name
        shutil.copy2(app, app_out)
        shutil.copy2(tmp_merged, merged_out)
        for p in (app_out, merged_out):
            h = hashlib.sha256(p.read_bytes()).hexdigest()
            lines.append(f"{h}  {p.name}")
        print("merge: wrote", merged_out, "size", merged_out.stat().st_size)

    try:
        tmp_merged.unlink()
    except OSError:
        pass

    (out_dir / "SHA256SUMS.txt").write_text("\n".join(lines) + "\n")
    dl = out_dir / "downloads"
    dl.mkdir(exist_ok=True)

env.AddPostAction("$BUILD_DIR/firmware.bin", after_build)
