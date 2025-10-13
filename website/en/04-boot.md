# Booting the Kernel

When a computer is turned on, the CPU initializes itself and starts executing the OS. OS initializes the hardware and starts the applications. This process is called "booting".

What happens before the OS starts? In PCs, BIOS (or UEFI in modern PCs) initializes the hardware, displays the splash screen, and loads the OS from the disk. In QEMU `virt` machine, OpenSBI is the equivalent of BIOS/UEFI.

## Supervisor Binary Interface (SBI)

The Supervisor Binary Interface (SBI) is an API for OS kernels, but defines what the firmware (OpenSBI) provides to an OS.

The SBI specification is [published on GitHub](https://github.com/riscv-non-isa/riscv-sbi-doc/releases). It defines useful features such as displaying characters on the debug console (e.g., serial port), reboot/shutdown, and timer settings.

A famous SBI implementation is [OpenSBI](https://github.com/riscv-software-src/opensbi). In QEMU, OpenSBI starts by default, performs hardware-specific initialization, and boots the kernel.

## Let's boot OpenSBI

First, let's see how OpenSBI starts. Create a shell script named `run.sh` as follows:

```
$ touch run.sh
$ chmod +x run.sh
```

```bash [run.sh]
#!/bin/bash
set -xue

# QEMU file path
QEMU=qemu-system-riscv32

# Start QEMU
$QEMU -machine virt -bios default -nographic -serial mon:stdio --no-reboot
```

QEMU takes various options to start the virtual machine. Here are the options used in the script:

- `-machine virt`: Start a `virt` machine. You can check other supported machines with the `-machine '?'` option.
- `-bios default`: Use the default firmware (OpenSBI in this case).
- `-nographic`: Start QEMU without a GUI window.
- `-serial mon:stdio`: Connect QEMU's standard input/output to the virtual machine's serial port. Specifying `mon:` allows switching to the QEMU monitor by pressing <kbd>Ctrl</kbd>+<kbd>A</kbd> then <kbd>C</kbd>.
- `--no-reboot`: If the virtual machine crashes, stop the emulator without rebooting (useful for debugging).

> [!TIP]
>
> In macOS, you can check the path to Homebrew's QEMU with the following command:
>
> ```
> $ ls $(brew --prefix)/bin/qemu-system-riscv32
> /opt/homebrew/bin/qemu-system-riscv32
> ```

Run the script and you will see the following banner:

```
$ ./run.sh

OpenSBI v1.2
   ____                    _____ ____ _____
  / __ \                  / ____|  _ \_   _|
 | |  | |_ __   ___ _ __ | (___ | |_) || |
 | |  | | '_ \ / _ \ '_ \ \___ \|  _ < | |
 | |__| | |_) |  __/ | | |____) | |_) || |_
  \____/| .__/ \___|_| |_|_____/|____/_____|
        | |
        |_|

Platform Name             : riscv-virtio,qemu
Platform Features         : medeleg
Platform HART Count       : 1
Platform IPI Device       : aclint-mswi
Platform Timer Device     : aclint-mtimer @ 10000000Hz
...
```

OpenSBI displays the OpenSBI version, platform name, features, number of HARTs (CPU cores), and more for debugging purposes.

When you press any key, nothing will happen. This is because QEMU's standard input/output is connected to the virtual machine's serial port, and the characters you type are being sent to the OpenSBI. However, no one reads the input characters.

Press <kbd>Ctrl</kbd>+<kbd>A</kbd> then <kbd>C</kbd> to switch to the QEMU debug console (QEMU monitor). You can exit QEMU by `q` command in the monitor:

```
QEMU 8.0.2 monitor - type 'help' for more information
(qemu) q
```

> [!TIP]
>
> <kbd>Ctrl</kbd>+<kbd>A</kbd> has several features besides switching to the QEMU monitor (<kbd>C</kbd> key). For example, pressing the <kbd>X</kbd> key will immediately exit QEMU.
>
> ```
> C-a h    print this help
> C-a x    exit emulator
> C-a s    save disk data back to file (if -snapshot)
> C-a t    toggle console timestamps
> C-a b    send break (magic sysrq)
> C-a c    switch between console and monitor
> C-a C-a  sends C-a
> ```

## Kernel package

We start creating the kernel package using Cargo.

```bash
$ cargo new kernel && cd kernel
```

This will create a subfolder `kernel`, as well as configuration files and an example Hello World Rust source file.

> [!TIP]
>
> If you are using `rust-analyzer`, you can edit the kernel Cargo.toml file to prevent spurious notifications, as
> we can't use "test", "doctest" or "bench" in our `no_std` environment.
> ``` [kernel/Config.toml]
> [package]
> name = "kernel"
> version = "0.1.0"
> edition = "2024"
>
> [[bin]]
> name = "kernel"
> test = false
> doctest = false
> bench = false
>
> [dependencies]
> ```
> 


## Linker script

A linker script is a file which defines the memory layout of executable files. Based on the layout, the linker assigns memory addresses to functions and variables.

Let's create a new file named `kernel.ld`:

```ld [kernel.ld]
ENTRY(boot)

SECTIONS {
    . = 0x80200000;

    .text :{
        KEEP(*(.text.boot));
        *(.text .text.*);
    }

    .rodata : ALIGN(4) {
        *(.rodata .rodata.*);
    }

    .data : ALIGN(4) {
        *(.data .data.*);
    }

    .bss : ALIGN(4) {
        __bss = .;
        *(.bss .bss.* .sbss .sbss.*);
        __bss_end = .;
    }

    . = ALIGN(4);
    . += 128 * 1024; /* 128KB */
    __stack_top = .;

   /DISCARD/ : { *(.eh_frame) }
}
```
Here are the key points of the linker script:

- The entry point of the kernel is the `boot` function.
- The base address is `0x80200000`.
- The `.text.boot` section is always placed at the beginning.
- Each section is placed in the order of `.text`, `.rodata`, `.data`, and `.bss`.
- The kernel stack comes after the `.bss` section, and its size is 128KB.
- Finally, we ask the linker to discard any `.eh_frame` using the `/DISCARD/` command, as this is only needed for stack unwinding, which we will not use. Rust creates this section automatically, and the linker script will put it at the beginning of our program (exactly where we want our boot text!) unless we say what to do.

`.text`, `.rodata`, `.data`, and `.bss` sections mentioned here are data areas with specific roles:

| Section   | Description                                                  |
| --------- | ------------------------------------------------------------ |
| `.text`   | This section contains the code of the program.               |
| `.rodata` | This section contains constant data that is read-only.       |
| `.data`   | This section contains read/write data.                       |
| `.bss`    | This section contains read/write data with an initial value of zero. |

Let's take a closer look at the syntax of the linker script. First, `ENTRY(boot)` declares that the `boot` function is the entry point of the program. Then, the placement of each section is defined within the `SECTIONS` block.

The `*(.text .text.*)` directive places the `.text` section and any sections starting with `.text.` from all files (`*`) at that location.

The `.` symbol represents the current address. It automatically increments as data is placed, such as with `*(.text)`. The statement `. += 128 * 1024` means "advance the current address by 128KB". The `ALIGN(4)` directive ensures that the current address is adjusted to a 4-byte boundary.

Finally, `__bss = .` assigns the current address to the symbol `__bss`. 

In Rust, you can refer to a defined symbol using 

```rust
unsafe extern "C" {
   static symbol_name: u8;
}
```

Here we use `unsafe` to mark that the Rust compiler is relying on our linker script to provide a valid symbol, it is `extern` to the Rust source files, the symbol is provided to Rust in `"C"` format, the symbol is `static` (i.e. unchanged for the life of program) and is an unsigned byte (`u8`). In our case we are not interested in the content of that byte, but rather the byte's memory address.

> [!TIP]
>
> Linker scripts offer many convenient features, especially for kernel development. You can find real-world examples on GitHub!

## Build script

We can tell Rust to use the linker script in a number of ways. We will use a Rust build script, which uses Rust code to guide the build process. Using a build script will give us flexibility later in this tutorial.

```rust [kernel/build.rs]
fn main() {
    // Add rustc linker arguments
    println!("cargo:rustc-link-arg=--script=kernel/kernel.ld");
    println!("cargo:rustc-link-arg=--Map=kernel/kernel.map");


    // Tell cargo to rerun if the linker script changes
    println!("cargo:rerun-if-changed=kernel.ld");
}
```

Rust build scripts use `println!()` macros to print commands for Cargo to follow. In this case, we print the linker arguments to ask the linker to use the `kernel.ld` linker script, and to output it's results in a `kernel.map` map file. We also tell Cargo to re-run if the linker script is changed (avoiding having to remember to use `cargo clean` each time we change the linker script.

## Minimal kernel

We're now ready to start writing the kernel. Let's start by creating a minimal one! Open the Rust language source code file named `src/main.rs`:

```rust [src/main.rs]
fn main() {
    println!("Hello, world!");
}
```
Clear the example code created by Cargo, and enter the code below.

```rust [src/main.rs]
//! OS in 1000 lines

#![no_std]
#![no_main]

use core::arch::naked_asm;
use core::panic::PanicInfo;
use core::ptr::write_bytes;

// Safety: Symbols created by linker script
unsafe extern "C" {
    static __bss: u8;
    static __bss_end: u8;
    static __stack_top: u8;
}

#[panic_handler]
fn panic(_panic: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
fn kernel_main() -> ! {
    let bss = &raw const __bss;
    let bss_end = &raw const __bss_end;
    // Safety: from linker script bss is aligned and bss segment is valid for writes up to bss_end
    unsafe {
        write_bytes(bss as *mut u8, 0, bss_end as usize - bss as usize);
    }

    loop {}
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
#[unsafe(naked)]
unsafe extern "C" fn boot() -> ! {
    naked_asm!(
        "la a0, {stack_top}",
        "mv sp, a0",
        "j {kernel_main}",
        stack_top = sym __stack_top,
        kernel_main = sym kernel_main,
    )
}
```

Let's explore the key points one by one:

### The kernel entry point

The execution of the kernel starts from the `boot` function, which is specified as the entry point in the linker script.

In this function, the stack pointer (`sp`) is set to the end address of the stack area defined in the linker script. Then, it jumps to the `kernel_main` function. It's important to note that the stack grows towards zero, meaning it is decremented as it is used. Therefore, the end address (not the start address) of the stack area must be set.

### `boot` function attributes

The `boot` function has three special attributes. The `naked` attribute instructs the compiler not to generate unnecessary code before and after the function body, such as a return instruction. This ensures that the inline assembly code is the exact function body.

The `boot` function also has the `link_section = ".text.boot"` attribute, which controls the placement of the function in the linker script. Since OpenSBI simply jumps to `0x80200000` without knowing the entry point, the `boot` function needs to be placed at `0x80200000`.

The `no_mangle` attribute stops the compiler from "mangling" the function name, so it just stays as `boot`. (This is not strictly necessary).

### Getting linker script symbols

At the beginning of the file, each symbol defined in the linker script is declared in an `extern "C"` block as a `static: symbol_name: u8;`. Here, we are only interested in obtaining the addresses of the symbols, so using `u8` type is not that important.

To get the address we use `&raw const`. This gives us the symbol's address without reading the byte (which at this point would be Undefined Behaviour).

> [TIP!]
> Writing safe Rust code allows us to avoid undefined behavour. However, as we are working on an operating system we are going to work with unsafe code, and we will need to take responsibility to avoid undefined behaviour ourselves.

### `.bss` section initialization

In the `kernel_main` function, the `.bss` section is first initialized to zero using the `write_bytes` function. Although some bootloaders may recognize and zero-clear the `.bss` section, but we initialize it manually just in case. Finally, the function enters an infinite loop and the kernel terminates.

### The panic handler

Rust requires a panic handler function to take care of any situation causing a panic. Usually this is provided by the standard library, but in our case we need to create it ourselves. For now we create a placeholder function which just loops, and give it the attribute `#[panic_handler]` so that Rust can use it for panics. Once we have the ability to print we will extend the function to provide useful messages. We put an underscore in front of the variable name to prevent Rust complaining about an unused variable.

## Let's run!

Add a kernel build command and a new QEMU option (`-kernel kernel.elf`) to `run.sh`:

```bash [run.sh] {6-8,12}
#!/bin/bash
set -xue

#QEMU file path
QEMU=qemu-system-riscv32

#Cargo will provide a path to the built kernel in $1
cp $1 kernel.elf

#Start QEMU
$QEMU -machine virt -bios default -nographic -serial mon:stdio --no-reboot \
    -kernel kernel.elf
```

## Your first kernel debugging

Run your kernel using Cargo.

```bash
$ cargo run
```
When you execute `cargo run`, the kernel enters an infinite loop. There are no indications that the kernel is running correctly. But don't worry, this is quite common in low-level development! This is where QEMU's debugging features come in.

To get more information about the CPU registers, open the QEMU monitor and execute the `info registers` command:

```
QEMU 9.2.4 monitor - type 'help' for more information
(qemu) info registers

CPU#0
 V      =   0
 pc       8020002e  ← Address of the instruction to be executed (Program Counter)
 ...
 x0/zero  00000000 x1/ra    8020002e x2/sp    8022008c x3/gp    00000000  ← Values of each register
 x4/tp    80046000 x5/t0    00000001 x6/t1    00000002 x7/t2    00001000
 x8/s0    80045f40 x9/s1    00000001 x10/a0   8020009c x11/a1   00000000
 x12/a2   00000000 x13/a3   8020009c x14/a4   8020009c x15/a5   00000001
 x16/a6   00000001 x17/a7   00000005 x18/s2   80200000 x19/s3   00000000
 x20/s4   87e00000 x21/s5   00000000 x22/s6   80006800 x23/s7   00000001
 x24/s8   00002000 x25/s9   80042308 x26/s10  00000000 x27/s11  00000000
 x28/t3   80020ad1 x29/t4   80045f40 x30/t5   000000b4 x31/t6   00000000
```

> [!TIP]
>
> The exact values may differ depending on the versions of Rust and QEMU.

`pc 8020002e` shows the current program counter, the address of the instruction being executed. Let's use the disassembler (`llvm-objdump`) to narrow down the specific line of code:

```
$ llvm-objdump -d kernel.elf

kernel.elf:     file format elf32-littleriscv

Disassembly of section .text:

80200000 <boot>:  ← boot function
80200000: 00020517      auipc   a0, 0x20
80200004: 09c50513      addi    a0, a0, 0x9c
80200008: 812a          mv      sp, a0
8020000a: 0040006f      j       0x8020000e <kernel_main>

8020000e <kernel_main>:
8020000e: 1141          addi    sp, sp, -0x10
80200010: c606          sw      ra, 0xc(sp)
80200012: 80200537      lui     a0, 0x80200
80200016: 09c50513      addi    a0, a0, 0x9c
8020001a: 80200637      lui     a2, 0x80200
8020001e: 09c60613      addi    a2, a2, 0x9c
80200022: 8e09          sub     a2, a2, a0
80200024: 4581          li      a1, 0x0
80200026: 00000097      auipc   ra, 0x0
8020002a: 00a080e7      jalr    0xa(ra) <memset>
8020002e: a001          j       0x8020002e <kernel_main+0x20> ← pc is here

80200030 <memset>:  ← write_bytes "memset" function
80200030: 46c1          li      a3, 0x10
80200032: 04d66f63      bltu    a2, a3, 0x80200090 <memset+0x60>
80200036: 40a006b3      neg     a3, a0
8020003a: 0036f813      andi    a6, a3, 0x3
8020003e: 01050733      add     a4, a0, a6
80200042: 00e57963      bgeu    a0, a4, 0x80200054 <memset+0x24>
80200046: 87c2          mv      a5, a6
80200048: 86aa          mv      a3, a0
8020004a: 00b68023      sb      a1, 0x0(a3)
8020004e: 17fd          addi    a5, a5, -0x1
80200050: 0685          addi    a3, a3, 0x1
80200052: ffe5          bnez    a5, 0x8020004a <memset+0x1a>
80200054: 41060633      sub     a2, a2, a6
80200058: ffc67693      andi    a3, a2, -0x4
8020005c: 96ba          add     a3, a3, a4
8020005e: 00d77e63      bgeu    a4, a3, 0x8020007a <memset+0x4a>
80200062: 0ff5f813      andi    a6, a1, 0xff
80200066: 010107b7      lui     a5, 0x1010
8020006a: 10178793      addi    a5, a5, 0x101
8020006e: 02f807b3      mul     a5, a6, a5
80200072: c31c          sw      a5, 0x0(a4)
80200074: 0711          addi    a4, a4, 0x4
80200076: fed76ee3      bltu    a4, a3, 0x80200072 <memset+0x42>
8020007a: 8a0d          andi    a2, a2, 0x3
8020007c: 00c68733      add     a4, a3, a2
80200080: 00e6f763      bgeu    a3, a4, 0x8020008e <memset+0x5e>
80200084: 00b68023      sb      a1, 0x0(a3)
80200088: 167d          addi    a2, a2, -0x1
8020008a: 0685          addi    a3, a3, 0x1
8020008c: fe65          bnez    a2, 0x80200084 <memset+0x54>
8020008e: 8082          ret
80200090: 86aa          mv      a3, a0
80200092: 00c50733      add     a4, a0, a2
80200096: fee567e3      bltu    a0, a4, 0x80200084 <memset+0x54>
8020009a: bfd5          j       0x8020008e <memset+0x5e>
```

Each line corresponds to an instruction. Each column represents:

- The address of the instruction.
- Hexadecimal dump of the machine code.
- Disassembled instructions.

`pc 8020002e` means the currently executed instruction is `j 0x8020002e <kernel_main+0x20>`. This confirms that QEMU has correctly reached the `kernel_main` function.

Our call to `core::ptr::write_bytes` has been translated by the compiler to `memset`. Rust uses LLVM, which in turn has some "intrinsic" functions to cover common functions, which you can see at [Crate core](https://doc.rust-lang.org/core/index.html). 

> [!TIP]
> We need to be careful about not reusing intrinsic's names in our function names. Creating a function called `memset` can confuse the compiler and create a recursive loop. 

Let's also check if the stack pointer (sp register) is set to the value of `__stack_top` defined in the linker script. The register dump shows `x2/sp 80220018`. To see where the linker placed `__stack_top`, check `kernel.map` file:

```
     VMA      LMA     Size Align Out     In      Symbol
       0        0 80200000     1 . = 0x80200000
80200000 80200000       9c     4 .text
...
80200000 80200000        e     1                 boot
...
8020000e 8020000e       22     1                 kernel_main
...
80200030 80200030       6c     1                 memset
...
8020009c 8020009c        0     4 .bss
8020009c 8020009c        0     1         __bss = .
8020009c 8020009c        0     1         __bss_end = .
8020009c 8020009c        0     1 . = ALIGN(4)
8020009c 8020009c    20000     1 . += 128 * 1024
8022009c 8022009c        0     1 __stack_top = .
```

`stack_top` starts at `8022009c`. If we look at the assembly our kernel starts with 
```
80200000 <boot>:  ← pc starts here
80200000: 00020517      auipc   a0, 0x20
80200004: 09c50513      addi    a0, a0, 0x9c
80200008: 812a          mv      sp, a0
...
8020000e <kernel_main>:
8020000e: 1141          addi    sp, sp, -0x10
```
The compiler has used `auipc` to add upper immediate to `pc`'s current value (which is `80200000`), giving us `0x80200000 + (0x20 << 12)`. Then it uses `addi` to add immediate `0x9c` to arrive at the stack top value `8022009c`. At the start of `kernel_main`, `-0x10` is added to `sp`, leaving us `8022008c` which is what we saw in the register!

Alternatively, you can also check the addresses of functions/variables using `llvm-nm`:

```
$ llvm-nm kernel.elf
00000000 N .Lline_table_start0
000000e3 N .Lline_table_start0
0000030e N .Lline_table_start1
80200000 t .Lpcrel_hi0
8020009c B __bss
8020009c B __bss_end
8022009c B __stack_top
80200000 T boot
8020000e T kernel_main
80200030 t memset
```

The first column is the address where they are placed (VMA). You can see that `__stack_top` is placed at `8022009c`. This confirms that the stack pointer is correctly set in the `boot` function. Nice!

As execution progresses, the results of `info registers` will change. If you want to temporarily stop the emulation, you can use the `stop` command in the QEMU monitor:

```
(qemu) stop             ← The process stops
(qemu) info registers   ← You can observe the state at the stop
(qemu) cont             ← The process resumes
```

Now you've successfully written your first kernel!
