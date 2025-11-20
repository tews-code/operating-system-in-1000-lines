#!/bin/bash
set -xue

#QEMU file path
QEMU=qemu-system-riscv32

#Cargo will provide a path to the built kernel in $1
cp $1 kernel.elf

(cd disk && tar cf ../disk.tar --format=ustar *.txt)

#     -d unimp,guest_errors,int,cpu_reset -D qemu.log \

#Start QEMU
$QEMU -machine virt -bios default -nographic -serial mon:stdio --no-reboot \
    -drive id=drive0,file=disk.tar,format=raw,if=none \
    -device virtio-blk-device,drive=drive0,bus=virtio-mmio-bus.0 \
    -kernel kernel.elf
