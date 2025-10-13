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

All we are doing here is creating an new *public* module called `print`. Cargo expects the contents of that module to be in a file called `print.rs`, so let's create `common/src/print.rs` as an empty file now.

Here's the implementation of the `printf` function:

```c [common.c]
#include "common.h"

void putchar(char ch);

void printf(const char *fmt, ...) {
    va_list vargs;
    va_start(vargs, fmt);

    while (*fmt) {
        if (*fmt == '%') {
            fmt++; // Skip '%'
            switch (*fmt) { // Read the next character
                case '\0': // '%' at the end of the format string
                    putchar('%');
                    goto end;
                case '%': // Print '%'
                    putchar('%');
                    break;
                case 's': { // Print a NULL-terminated string.
                    const char *s = va_arg(vargs, const char *);
                    while (*s) {
                        putchar(*s);
                        s++;
                    }
                    break;
                }
                case 'd': { // Print an integer in decimal.
                    int value = va_arg(vargs, int);
                    unsigned magnitude = value; // https://github.com/nuta/operating-system-in-1000-lines/issues/64
                    if (value < 0) {
                        putchar('-');
                        magnitude = -magnitude;
                    }

                    unsigned divisor = 1;
                    while (magnitude / divisor > 9)
                        divisor *= 10;

                    while (divisor > 0) {
                        putchar('0' + magnitude / divisor);
                        magnitude %= divisor;
                        divisor /= 10;
                    }

                    break;
                }
                case 'x': { // Print an integer in hexadecimal.
                    unsigned value = va_arg(vargs, unsigned);
                    for (int i = 7; i >= 0; i--) {
                        unsigned nibble = (value >> (i * 4)) & 0xf;
                        putchar("0123456789abcdef"[nibble]);
                    }
                }
            }
        } else {
            putchar(*fmt);
        }

        fmt++;
    }

end:
    va_end(vargs);
}
```

It's surprisingly concise, isn't it? It goes through the format string character by character, and if we encounter a `%`, we look at the next character and perform the corresponding formatting operation. Characters other than `%` are printed as is.

For decimal numbers, if `value` is negative, we first output a `-` and then get its absolute value. We then calculate the divisor to get the most significant digit and output the digits one by one. We use `unsigned` for `magnitude` to handle `INT_MIN` case. See [this issue](https://github.com/nuta/operating-system-in-1000-lines/issues/64) for more details.

For hexadecimal numbers, we output from the most significant *nibble* (a hexadecimal digit, 4 bits) to the least significant. Here, `nibble` is an integer from 0 to 15, so we use it as the index in string `"0123456789abcdef"` to get the corresponding character.

`va_list` and related macros are defined in the C standard library's `<stdarg.h>`. In this book, we use compiler builtins directly without relying on the standard library. Specifically, we'll define them in `common.h` as follows:

```c [common.h]
#pragma once

#define va_list  __builtin_va_list
#define va_start __builtin_va_start
#define va_end   __builtin_va_end
#define va_arg   __builtin_va_arg

void printf(const char *fmt, ...);
```

We're simply defining these as aliases for the versions with `__builtin_` prefixed. They are builtin features provided by the compiler (clang) itself ([Reference: clang documentation](https://clang.llvm.org/docs/LanguageExtensions.html#variadic-function-builtins)). The compiler will handle the rest appropriately, so we don't need to worry about it.

Now we've implemented `printf`. Let's add a "Hello World" from the kernel:

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
