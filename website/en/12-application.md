# Application

In this chapter, we'll prepare the first application executable to run on our kernel. First let's add a new package to our workspace, which will be the user library for applications.

```
$ cargo new --lib user
    Creating library `user` package
      Adding `user` as member of workspace
```

Now add `user` as a dependency in our workspace by edit the project root Cargo.toml: 

```
[workspace]
members = ["common","kernel", "user"]
resolver = "3"

[workspace.dependencies]
common = { path = "common" }
user = { path = "user" }
```
as well as adding `common` as a dependency for `user` in the `user/Cargo.toml`:
```toml [user/Cargo.toml]
[package]
name = "user"
version = "0.1.0"
edition = "2024"

[lib]
test = false
doctest = false
bench = false

[dependencies]
common = { workspace = true }
```
Add our cross-compile target into `user/.cargo/config.toml`: 

```toml [user/.cargo/config.toml]
[build]
target="riscv32i-unknown-none-elf"
```
and add a build script to make use of a linker script and to provide a linker map for user applications in `user/build.rs`:

```rust [user/build.rs]
fn main() {
    println!("cargo:rustc-link-arg=--Map=user/user.map");
    println!("cargo:rustc-link-arg=--script=user/user.ld");
}
```

## Memory layout

In the previous chapter, we implemented isolated virtual address spaces using the paging mechanism. Let's  consider where to place the application in the address space.

Create a new linker script (`user.ld`) that defines where to place the application in memory:

```ld [user/user.ld]
ENTRY(start)

SECTIONS {
    . = 0x1000000;

    /* machine code */
    .text :{
        KEEP(*(.text.start));
        *(.text .text.*);
    }

    /* read-only data */
    .rodata : ALIGN(4) {
        *(.rodata .rodata.*);
    }

    /* data with initial values */
    .data : ALIGN(4) {
        *(.data .data.*);
    }

    /* data that should be zero-filled at startup */
    .bss : ALIGN(4) {
        *(.bss .bss.* .sbss .sbss.*);

        . = ALIGN(16);
        . += 64 * 1024; /* 64KB */
        __user_stack_top = .;

       ASSERT(. < 0x1800000, "too large executable");
    }

    /DISCARD/ : { *(.eh_frame*) }
}
```

It looks pretty much the same as the kernel's linker script, isn't it?  The key difference is the base address (`0x1000000`) so that the application doesn't overlap with the kernel's address space.

`ASSERT` is an assertion which aborts the linker if the condition in the first argument is not met. Here, it ensures that the end of the `.bss` section, which is the end of the application memory, does not exceed `0x1800000`. This is to ensure that the executable file doesn't accidentally become too large.

## Userland library

Next, let's create a library for userland programs. For simplicity, we'll start with a minimal feature set to start the application. Clear the Cargo example code, and add the following:

```rust [user/src/lib.rs]
//! User library for os1k

#![no_std]

use core::arch::naked_asm;
use core::panic::PanicInfo;

pub use common::{print, println};

#[panic_handler]
pub fn panic(_panic: &PanicInfo) -> ! {
    loop {}
}

unsafe extern "C" {
    static __user_stack_top: u8;
}

#[unsafe(no_mangle)]
fn exit() -> ! {
    loop {}
}

#[unsafe(link_section = ".text.start")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
unsafe extern "C" fn start() {
    naked_asm!(
        "la sp, {stack_top}",
        "call main",
        "call exit",
        stack_top = sym __user_stack_top
    )
}
```
The execution of the application starts from the `start` function. Similar to the kernel's boot process, it sets up the stack pointer and calls the application's `main` function.

We prepare the `exit` function to terminate the application. However, for now, we'll just have it perform an infinite loop.

Unlike the kernel's initialization process, we don't clear the `.bss` section with zeros. This is because the kernel guarantees that it has already filled it with zeros (in the `alloc_pages` function).

> [!TIP]
>
> Allocated memory regions are already filled with zeros in typical operating systems too. Otherwise, the memory may contain sensitive information (e.g. credentials) from other processes, and it could lead to a critical security issue.

## First application

It's time to create the first application! Unfortunately, we still don't have a way to display characters, we can't start with a "Hello, World!" program. Instead, we'll create a simple infinite loop.

Create a subfolder `user/src/bin` and add the file `shell.rs`.

```rust [user/src/bin/shell.rs]
//! os1k shell

#![no_std]
#![no_main]

use core::arch::asm;

#[expect(unused_imports)]
use user;

#[unsafe(no_mangle)]
fn main() {
    loop {}
}
```

## Building the application

Applications will be built separately from the kernel, converted to a "relocatable ELF" and linked directly to our kernel binary.

First, we need to add this to our kernel build script at `kernel/build.rs`:

```rust [kernel/build.rs] {9-10}
fn main() {
    // Add rustc linker arguments
    println!("cargo:rustc-link-arg=--Map=kernel/kernel.map");
    println!("cargo:rustc-link-arg=--script=kernel/kernel.ld");

    // Tell cargo to rerun if the linker script changes
    println!("cargo:rerun-if-changed=kernel.ld");

    // Link the shell binary
    println!("cargo:rustc-link-arg=shell.bin.o");
}
```
and at the same time, let's tell Cargo which binary is our default by editing `kernel/Cargo.toml`:

```toml [kernel/Cargo.toml] {5}
[package]
name = "kernel"
version = "0.1.0"
edition = "2024"
default-run = "kernel"

[[bin]]
name = "kernel"
test = false
doctest = false
bench = false

[dependencies]
common = { workspace = true }
```

Let's create a new script (`os1k.sh`) to control building the application, converting it to a relocatable binary, and then building and running the kernel:

```bash [os1k.sh]
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
```
The first `$OBJCOPY` command converts the executable file (in ELF format) to raw binary format. A raw binary is the actual content that will be expanded in memory from the base address (in this case, `0x1000000`). The OS can prepare the application in memory simply by copying the contents of the raw binary. Common OSes use formats like ELF, where memory contents and their mapping information are separate, but in this book, we'll use raw binary for simplicity.

The second `$OBJCOPY` command converts the raw binary execution image into a format that can be embedded in our Rust kernel binary. Let's take a look at what's inside using the `llvm-nm` command:

```
$ llvm-nm shell.bin.o
00010020 D _binary_shell_bin_end
00010020 A _binary_shell_bin_size
00000000 D _binary_shell_bin_start
```

The prefix `_binary_` is followed by the file name, and then `start`, `end`, and `size`. These are symbols that indicate the beginning, end, and size of the execution image, respectively. In practice, they are used as follows:

```c
extern char _binary_shell_bin_start[];
extern char _binary_shell_bin_size[];

void main(void) {
    uint8_t *shell_bin = (uint8_t *) _binary_shell_bin_start;
    printf("shell_bin size = %d\n", (int) _binary_shell_bin_size);
    printf("shell_bin[0] = %x (%d bytes)\n", shell_bin[0]);
}
```

This program outputs the file size of `shell.bin` and the first byte of its contents. In other words, you can treat the `_binary_shell_bin_start` variable as if it contains the file contents, like:

```c
char _binary_shell_bin_start[] = "<shell.bin contents here>";
```

`_binary_shell_bin_size` variable contains the file size. However, it's used in a slightly unusual way. Let's check with `llvm-nm` again:

```
$ llvm-nm shell.bin.o | grep _binary_shell_bin_size
00010454 A _binary_shell_bin_size

$ ls -al target/riscv32imac-unknown-none-elf/debug/shell.bin ← note: do not confuse with shell.bin.o!
-rwxr-xr-x. 1 65568 Oct 20 17:45 target/riscv32imac-unknown-none-elf/debug/shell.bin

$ python3 -c 'print(0x10020)'
65568
```

The first column in the `llvm-nm` output is the *address* of the symbol. This `10020` hexadecimal number matches the file size, but this is not a coincidence. Generally, the values of each address in a `.o` file are determined by the linker. However, `_binary_shell_bin_size` is special.

The `A` in the second column indicates that the address of `_binary_shell_bin_size` is a type of symbol (absolute) that should not be changed by the linker. That is, it embeds the file size as an address.

By defining it as an array of an arbitrary type like `char _binary_shell_bin_size[]`, `_binary_shell_bin_size` will be treated as a pointer storing its *address*. However, since we're embedding the file size as an address here, casting it will result in the file size. This is a common trick (or a dirty hack) that exploits the object file format.

Lastly, we've added `shell.bin.o` to the `build.rs` script in the kernel compiling. It embeds the first application's executable into the kernel image.

## Disassemble the executable

In disassembly, we can see that the `.text.start` section is placed at the beginning of the executable file. The `start` function should be placed at `0x1000000` as follows:

```
$ lllvm-objdump -d target/riscv32imac-unknown-none-elf/debug/shell

target/riscv32imac-unknown-none-elf/debug/shell:        file format elf32-littleriscv

Disassembly of section .text:

01000000 <start>:
 1000000: 00010117      auipc   sp, 0x10
 1000004: 02010113      addi    sp, sp, 0x20
 1000008: 00000097      auipc   ra, 0x0
 100000c: 010080e7      jalr    0x10(ra) <main>
 1000010: 00000097      auipc   ra, 0x0
 1000014: 00a080e7      jalr    0xa(ra) <exit>

01000018 <main>:
 1000018: a001          j       0x1000018 <main>

0100001a <exit>:
 100001a: a001          j       0x100001a <exit>
```
