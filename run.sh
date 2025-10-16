#!/bin/bash
set -xue

#QEMU file path
QEMU=qemu-system-riscv32

#Cargo will provide a path to the built kernel in $1
cp $1 kernel.elf

#Start QEMU
$QEMU -machine virt -bios default -nographic -serial mon:stdio --no-reboot \
    -kernel kernel.elf
