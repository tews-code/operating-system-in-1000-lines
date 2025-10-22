#!/bin/bash
set -xue

TARGET=riscv32imac-unknown-none-elf
TARGET_DIR=target/$TARGET/debug/
OBJCOPY=llvm-objcopy
CWD=$(pwd)

echo $CWD;

if [ $1 == "clean" ]; then
    cargo clean;
    rm -f shell.bin;
    rm -f shell.bin.o;
fi


if [ $1 == "check" ]; then
    cargo check -p user --bin shell;
    cargo build -p user --bin shell;
    cd $TARGET_DIR;
    $OBJCOPY --set-section-flags=.bss=alloc,contents \
        --output-target=binary \
        shell shell.bin;
    cp shell.bin $CWD;
    $OBJCOPY -Ibinary -Oelf32-littleriscv shell.bin shell.bin.o;
    file shell.bin.o;
    cp shell.bin.o $CWD;
    cd $CWD;
    cargo check --bin kernel;
fi

if [ $1 == "build" ]; then
    cargo build -p user --bin shell;
    cd $TARGET_DIR;
    $OBJCOPY --set-section-flags=.bss=alloc,contents \
        --output-target=binary \
        shell shell.bin;
    # For build, let's make a copy of shell.bin in case of debugging
    cp shell.bin $CWD;
    $OBJCOPY -Ibinary -Oelf32-littleriscv shell.bin shell.bin.o;
    file shell.bin.o;
    cp shell.bin.o $CWD;
    cd $CWD;
    cargo build --bin kernel;
fi

if [ $1 == "run" ]; then
    if [ -f $TARGET/shell.bin.o ]; then
        cargo run;
    else
        ./$0 build;
        cargo run;
    fi
fi

if [ $1 == "cleanandrun" ]; then
    ./$0 clean;
    ./$0 run;
fi
