# Hello World! 

In the previous chapter, we successfully booted our first kernel. Although we could confirm it works by reading the register dump, it still felt somewhat unsatisfactory.

In this chapter, let's make it more obvious by outputting a string from the kernel.

## Say "hello" to SBI

In the previous chapter, we learned that SBI is an "API for OS". To call the SBI to use its function, we use the `ecall` instruction. Create a new file `kernel/src/sbi.rs`.

```rust [kernel/src/sbi.rs]
//! SBI Interface

use core::arch::asm;
use core::ffi::{c_long, c_int};

const EID_CONSOLE_PUTCHAR: c_long = 1;

// Safety: Caller must ensure that SBI call does not change machine state, memory mappings etc.
unsafe fn sbi_call(arg0: c_int, eid: c_long) -> Result<isize, isize> {
    let result: c_long;
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") arg0 => result,
            in("a7") eid,
        );
    }
    match result {
        0 => Ok(0),
        _ => Err(result as isize),
    }
}

#[unsafe(no_mangle)]
pub fn put_byte(b: u8) -> Result<isize, isize> {
    // Safety: EID_CONSOLE_PUTCHAR is a safe SBI call that only writes to console
    unsafe {
        sbi_call(b as c_int, EID_CONSOLE_PUTCHAR)
    }
}
```

Because OpenSBI is has API using C code, we use the Rust module for a Foreign Function Interface, and use C variable types. In our case, we use `c_int` which is equivalent to Rust's `i32`, and `c_long` which is also equivalent to Rust's `i32`. Since we are using a 32-bit platform, we can cast these as `isize` without concern. 

First we create a constant representing the SBI Extension ID "Console Putchar", which puts a byte on the debug console.

We then create a function `sbi_call` that uses assembly to make the `ecall` to the SBI. This function looks at the result of the SBI call and transforms this into a Rust `Result<>`. The assembly uses `inlateout` to tell the compiler that register `"a0"` first has a value, and is then clobbered by `result`.

Finally, we create a function to safely make the SBI call. This function is safe, even though the underlying code has an unsafe function call, so we can confidently use this without being concerned about undefined behaviour.

```rust [kernel/src/main.rs] {6, 10, 32-35}
//! OS in 1000 lines

#![no_std]
#![no_main]

use core::arch::{asm, naked_asm};
use core::panic::PanicInfo;
use core::ptr::write_bytes;

mod sbi;

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

    for b in "\n\nHello World!\n".bytes() {
      let _ =  sbi::put_byte(b);
    };

    loop {
        unsafe{asm!("wfi");}
    }
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
    );
}
```

We've newly added the `sbi_call` function. This function is designed to call OpenSBI as specified in the SBI specification. The specific calling convention is as follows:

> **Chapter 5. Legacy Extensions (EIDs #0x00 - #0x0F)**
>
> - An `ECALL` is used as the control transfer instruction between the supervisor and the SEE.
> - `a7` encodes the SBI extension ID (**EID**).
>
> The legacy SBI extensions follow a slightly different calling convention as compared to the SBI v0.2
> (or higher) specification where:
> - The SBI function ID field in `a6` register is ignored because these are encoded as multiple SBI extension IDs.
> - Nothing is returned in `a1` register.
> - All registers except `a0` must be preserved across an SBI call by the callee.
> - The value returned in `a0` register is SBI legacy extension specific.
>
> The page and access faults taken by the SBI implementation while accessing memory on behalf of the
supervisor are redirected back to the supervisor with sepc CSR pointing to the faulting `ECALL`
instruction.
> The legacy console SBI functions (`sbi_console_getchar()` and `sbi_console_putchar()`) are expected to be deprecated; they have no replacement.
>
> -- "RISC-V Supervisor Binary Interface Specification" Version 1.0.0, March 22, 2022: Ratified

> [!TIP]
>
> *"All registers except `a0` must be preserved across an SBI call by the callee."* means that the callee (OpenSBI side) must not change the values of ***except*** `a0`. In other words, from the kernel's perspective, it is guaranteed that the registers (`a2` to `a7`) will remain the same after the call.

The `in("register name")` in the `asm!` macro used in each local variable declaration asks the compiler to place values in the specified registers. This is a common idiom in system call invocations (e.g., [Linux system call invocation process](https://git.musl-libc.org/cgit/musl/tree/arch/riscv64/syscall_arch.h)).

After preparing the arguments, the `ecall` instruction is executed in inline assembly. When this is called, the CPU's execution mode switches from kernel mode (S-Mode) to OpenSBI mode (M-Mode), and OpenSBI's processing handler is invoked. Once it's done, it switches back to kernel mode, and execution resumes after the `ecall` instruction. 

The `ecall` instruction is also used when applications call the kernel (system calls). This instruction behaves like a function call to the more privileged CPU mode.

To display characters, we can use `Console Putchar` function:

> 5.2. Extension: Console Putchar (EID #0x01)
>
> ```c
>   long sbi_console_putchar(int ch)
> ```
>
> Write data present in ch to debug console.
>
> Unlike sbi_console_getchar(), this SBI call will block if there remain any pending characters to be transmitted or if the receiving terminal is not yet ready to receive the byte. However, if the console doesn’t exist at all, then the character is thrown away.
>
> This SBI call returns 0 upon success or an implementation specific negative error code.
>
> -- "RISC-V Supervisor Binary Interface Specification" v1.0.0

`Console Putchar` is a function that outputs the character passed as an argument to the debug console.


### Try it out

Let's try your implementation. You should see `Hello World!` if it works:

```
$ cargo run
...


Hello World!
```

> [!TIP]
>
> **Life of Hello World:**
>
> When SBI is called, characters will be displayed as follows:
>
> 1. The kernel executes `ecall` instruction. The CPU jumps to the M-mode trap handler (`mtvec` register), which is set by OpenSBI during startup.
> 2. After saving registers, the [trap handler written in C](https://github.com/riscv-software-src/opensbi/blob/0ad866067d7853683d88c10ea9269ae6001bcf6f/lib/sbi/sbi_trap.c#L263) is called.
> 3. Based on the `eid`, the [corresponding SBI processing function is called](https://github.com/riscv-software-src/opensbi/blob/0ad866067d7853683d88c10ea9269ae6001bcf6f/lib/sbi/sbi_ecall_legacy.c#L63C2-L65).
> 4. The [device driver](https://github.com/riscv-software-src/opensbi/blob/0ad866067d7853683d88c10ea9269ae6001bcf6f/lib/utils/serial/uart8250.c#L77) for the 8250 UART ([Wikipedia](https://en.wikipedia.org/wiki/8250_UART)) sends the character to QEMU.
> 5. QEMU's 8250 UART emulation implementation receives the character and sends it to the standard output.
> 6. The terminal emulator displays the character.
>
> That is, by calling `Console Putchar` function is not a magic at all - it just uses the device driver implemented in OpenSBI!

## `println!` macro

We've successfully printed some characters. The next item is implementing `println!` macro.

`println!` macro takes a format string, and the values to be embedded in the output. For example, `println!("1 + 2 = {}", 1 + 2)` will display `1 + 2 = 3`.

While `println!` bundled in the Rust standard library, it is not provided in the `core` Rust library, so we will need to implement this. Specifically, we'll implement a `println!` and a "print!" which does not add a new line character.

Since we'll use `println!` in applications too, let's create a new package `common` for code shared between the kernel and userland.

## `common` package

Let's go to our project root folder, and use Cargo to create a new library package.

```bash
$ cd ..
$ cargo new --lib common
```
Cargo will create the new package and populate an example library source file, as well as automatically adding it as a member of our workspace.

Open `common/src/lib.rs`, clear the example code and replace it with:

```rust [common/src/lib.rs]
//! Common library

#![no_std]

pub mod print;
```
> [TIP!]
> As before, if you want to avoid spurious `rust-analyzer` warnings, you can add
> ```toml [common/Cargo.toml]
> [lib]
> test = false
> doctest = false
> bench = false
> ```
> to the `common` package `Cargo.toml`.

All we are doing here is creating an new *public* module called `print`. Cargo expects the contents of that module to be in a file called `print.rs`, so let's create `common/src/print.rs` as an empty file now.

Here's the implementation of the `println!` macro:

```rust [common/src/print.rs]
//! Print to debug console

use core::fmt;

pub struct DebugConsole;

unsafe extern "Rust" {
    pub fn put_byte(b: u8) -> Result<isize, isize>;
}

impl fmt::Write for DebugConsole {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            unsafe { put_byte(b).map_err(|_| fmt::Error)?; }
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        {
            use core::fmt::Write;
            let _ = write!($crate::print::DebugConsole, $($arg)*);
        }
    };
}

#[macro_export]
macro_rules! println {
    () => { $crate::print!("\n") }; // Allows us to use println!() to print a newline.
    ($($arg:tt)*) => {
        {
            use core::fmt::Write;
            let _ = writeln!($crate::print::DebugConsole, $($arg)*);
        }
    };
}
```

C programming supports functions with variable numbers of arguments ("variadics"), but in Rust we need to use macros to achieve the same thing. We need this for printing so that we can print strings with different numbers of substitutions, like `println!("This is just a string literal.");` and `println!("{} + {} = {}", 1, 2, 3);`. Fortunately we can simply wrap the provided `write!` macros in our own `print!` and `println!` macros, meaning most of the work is already done for us.

Firstly we let the compiler know that the `put_byte` function will be provided by an external package, and share the expected signature.

We then declare zero-sized sttuct `pub struct DebugConsole`, and then we attach the `core::fmt::Write` trait to our struct. The Write trait has one mandatory function that we need to provide called `write_str`, and once that is in place it will provide the `write!` and `writeln!` macros.

The `write_str` function needs to take a string and print it out - just what our `put_byte` function can do! It must return a `core::fmt::Result`, which is defined as `Result<(), core::fmt::Error>, so we map any errors (`|_|`) from our `put_byte` call to `core::fmt::Error` instead, or pass an empty `Ok(())` on success. 

Rust macros are similar to `match` statements - if the rule on the left matches, then the expansion on the right is used. Here you can see we are allowing for repetitions with `*` - meaning zero or more repetitions - around a variable `$arg` which is a *metavariable* of type *token tree* (`tt`). A token tree can match almost anything, including our variable number of arguments.

Once we match the token tree on the left, we expand the expression on the right. Here we are simply calling the `write!` or `writeln!` macros from the Write trait, passing on our `$arg` repetitions.

On error our `print!` and `println!` macros will simply panic thanks to `unwrap()`, as at this point there is little else we can do.

We use the `#[macro_export]` attribute to export the macros to the root crate (the root of our kernel module), so they can be used throughout the kernel.

Now we've implemented the `println!` macro. Let's add a "Hello World" from the kernel:

```rust [kernel/main.rs] {10-11, 26-27}
//! OS in 1000 lines

#![no_std]
#![no_main]

use core::arch::{asm, naked_asm};
use core::panic::PanicInfo;
use core::ptr::write_bytes;

#[allow(unused_imports)]
use common::{print, println};

mod sbi;

...

#[unsafe(no_mangle)]
fn kernel_main() -> ! {
    let bss = &raw const __bss;
    let bss_end = &raw const __bss_end;
    // Safety: from linker script bss is aligned and bss segment is valid for writes up to bss_end
    unsafe {
        write_bytes(bss as *mut u8, 0, bss_end as usize - bss as usize);
    }

    println!("Hello world! 🦀");
    println!("1 + 2 = {}, 0x{:x}", 1 + 2, 0x1234abcd);

    loop {
        unsafe{asm!("wfi");}
    }
}
...
```
```c [kernel.c] {2,5-6}
#include "kernel.h"
#include "common.h"

void kernel_main(void) {
    printf("\n\nHello %s\n", "World!");
    printf("1 + 2 = %d, %x\n", 1 + 2, 0x1234abcd);

    for (;;) {
        __asm__ __volatile__("wfi");
    }
}
```

Also, Add `common.c` to the compilation targets:

```bash [run.sh] {2}
$CC $CFLAGS -Wl,-Tkernel.ld -Wl,-Map=kernel.map -o kernel.elf \
    kernel.c common.c
```

Now, let's try! You will see `Hello World!` and `1 + 2 = 3, 1234abcd` as shown below:

```
$ ./run.sh

Hello World!
1 + 2 = 3, 1234abcd
```

The powerful ally "printf debugging" has joined your OS!
