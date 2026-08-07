#!/usr/bin/env bash
# Spot-check a built Pi Invest OS image (rootfs + boot FAT).
set -euo pipefail

IMG="${1:?usage: $0 path/to/pi-invest-os-*.img}"
[[ -f "$IMG" ]] || { echo "missing $IMG"; exit 1; }

need_sudo() { [[ $EUID -eq 0 ]] || exec sudo -E bash "$0" "$@"; }
need_sudo "$@"

export MTOOLS_SKIP_CHECK=1
LOOP="$(losetup -f --show -P "$IMG")"
cleanup() { umount "$MNT" 2>/dev/null || true; losetup -d "$LOOP" 2>/dev/null || true; }
trap cleanup EXIT
partprobe "$LOOP" 2>/dev/null || true
for _ in $(seq 1 40); do [[ -b "${LOOP}p2" ]] && break; sleep 0.2; done

MNT="$(mktemp -d)"
mount "${LOOP}p2" "$MNT"
echo "version: $(cat "$MNT/etc/pi-invest-os-version")"
test -f "$MNT/opt/pi-invest-agent/pyproject.toml"
test -x "$MNT/usr/local/sbin/pi-invest-firstboot.sh"
test -L "$MNT/etc/systemd/system/multi-user.target.wants/pi-invest-firstboot.service"
echo "rootfs: OK"
umount "$MNT"
mdir -i "${LOOP}p1" :: | grep -qi 'pi-invest.env'
mdir -i "${LOOP}p1" :: | grep -qi 'userconf'
mdir -i "${LOOP}p1" :: | grep -qi 'ssh'
echo "boot: OK"
echo "VERIFY_OK $IMG"
