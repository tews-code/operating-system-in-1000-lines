#!/bin/bash
set -xue

TARGET=riscv32imac-unknown-none-elf
TARGET_DIR=target/$TARGET/debug/
OBJCOPY=llvm-objcopy
CWD=$(pwd)

# Set default command if none provided
COMMAND=${1:-run}

if [ "$COMMAND" == "clean" ]; then
    cargo clean;
    rm -f kernel.elf;
    rm -f disk.tar;
    rm -f shell.bin;
    rm -f shell.bin.o;
    rm -f kernel/kernel.map;
    rm -f user/user.map;
fi


if [ "$COMMAND" == "check" ]; then
    cargo check -p user --bin shell;
    cargo build -p user --bin shell;
    cd $TARGET_DIR;
    $OBJCOPY --set-section-flags=.bss=alloc,contents \
        --output-target=binary \
        shell shell.bin;
    cp shell.bin "$CWD";
    $OBJCOPY -Ibinary -Oelf32-littleriscv shell.bin shell.bin.o;
    file shell.bin.o;
    cp shell.bin.o "$CWD";
    cd "$CWD";
    cargo check --bin kernel;
fi

if [ "$COMMAND" == "build" ]; then
    cargo build -p user --bin shell;
    cd $TARGET_DIR;
    $OBJCOPY --set-section-flags=.bss=alloc,contents \
        --output-target=binary \
        shell shell.bin;
    # For build, let's make a copy of shell.bin in case of debugging
    cp shell.bin "$CWD";
    $OBJCOPY -Ibinary -Oelf32-littleriscv shell.bin shell.bin.o;
    file shell.bin.o;
    cp shell.bin.o "$CWD";
    cd "$CWD";
    cargo build --bin kernel;
fi

if [ "$COMMAND" == "run" ]; then
    if [ -f $TARGET/shell.bin.o ]; then
        cargo run;
    else
        "./$0" build;
        cargo run;
    fi
fi

if [ "$COMMAND" == "cleanandrun" ]; then
    "./$0" clean;
    "./$0" run;
fi
