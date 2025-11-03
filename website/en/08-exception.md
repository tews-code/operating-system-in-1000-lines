# Exception

Exception is a CPU feature that allows the kernel to handle various events, such as invalid memory access (aka. page faults), illegal instructions, and system calls.

Exception is like a hardware-assisted `try-catch` mechanism in C++ or Java. Until CPU encounters the situation where kernel intervention is required, it continues to execute the program. The key difference from `try-catch` is that the kernel can resume the execution from the point where the exception occurred, as if nothing happened. Doesn't it sound like cool CPU feature?

Exception can also be triggered in kernel mode and mostly they are fatal kernel bugs. If QEMU resets unexpectedly or the kernel does not work as expected, it's likely that an exception occurred. I recommend to implement an exception handler early to crash gracefully with a kernel panic. It's similar to adding an unhandled rejection handler as the first step in JavaScript development.

## Life of an exception

In RISC-V, an exception will be handled as follows:

1. CPU checks the `medeleg` register to determine which operation mode should handle the exception. In our case, OpenSBI has already configured to handle U-Mode/S-mode exceptions in S-Mode's handler.
2. CPU saves its state (registers) into various CSRs (see below).
3. The value of the `stvec` register is set to the program counter, jumping to the kernel's exception handler.
4. The exception handler saves general-purpose registers (i.e. the program state), and handles the exception.
5. Once it's done, the exception handler restores the saved execution state and calls the `sret` instruction to resume execution from the point where the exception occurred.

The CSRs updated in step 2 are mainly as follows. The kernel's exception determines necessary actions based on the CSRs:

| Register Name | Content                                                                                                                                         |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `scause`      | Type of exception. The kernel reads this to identify the type of exception.                                                                     |
| `stval`       | Additional information about the exception (e.g., memory address that caused the exception). Depends on the type of exception. |
| `sepc`        | Program counter at the point where the exception occurred.                                                                                       |
| `sstatus`     | Operation mode (U-Mode/S-Mode) when the exception has occurred.                                                                                        |

## Exception Handler

Now let's write your first exception handler! Create a new file `entry.rs` and add it as a module in `main.rs`. 

Here's the entry point of the exception handler to be registered in the `stvec` register:

```rust [kernel/src/entry.rs]
#[unsafe(naked)]
pub unsafe extern "C" fn kernel_entry() {
    naked_asm!(
        ".align 2",
        // Retrieve the kernel stack of the running process from sscratch.
        "csrw sscratch, sp",
        "addi sp, sp, -4 * 31",
        "sw ra,  4 * 0(sp)",
        "sw gp,  4 * 1(sp)",
        "sw tp,  4 * 2(sp)",
        "sw t0,  4 * 3(sp)",
        "sw t1,  4 * 4(sp)",
        "sw t2,  4 * 5(sp)",
        "sw t3,  4 * 6(sp)",
        "sw t4,  4 * 7(sp)",
        "sw t5,  4 * 8(sp)",
        "sw t6,  4 * 9(sp)",
        "sw a0,  4 * 10(sp)",
        "sw a1,  4 * 11(sp)",
        "sw a2,  4 * 12(sp)",
        "sw a3,  4 * 13(sp)",
        "sw a4,  4 * 14(sp)",
        "sw a5,  4 * 15(sp)",
        "sw a6,  4 * 16(sp)",
        "sw a7,  4 * 17(sp)",
        "sw s0,  4 * 18(sp)",
        "sw s1,  4 * 19(sp)",
        "sw s2,  4 * 20(sp)",
        "sw s3,  4 * 21(sp)",
        "sw s4,  4 * 22(sp)",
        "sw s5,  4 * 23(sp)",
        "sw s6,  4 * 24(sp)",
        "sw s7,  4 * 25(sp)",
        "sw s8,  4 * 26(sp)",
        "sw s9,  4 * 27(sp)",
        "sw s10, 4 * 28(sp)",
        "sw s11, 4 * 29(sp)",

        "csrr a0, sscratch",
        "sw a0, 4 * 30(sp)",

        "mv a0, sp",
        "call handle_trap",

        "lw ra,  4 * 0(sp)",
        "lw gp,  4 * 1(sp)",
        "lw tp,  4 * 2(sp)",
        "lw t0,  4 * 3(sp)",
        "lw t1,  4 * 4(sp)",
        "lw t2,  4 * 5(sp)",
        "lw t3,  4 * 6(sp)",
        "lw t4,  4 * 7(sp)",
        "lw t5,  4 * 8(sp)",
        "lw t6,  4 * 9(sp)",
        "lw a0,  4 * 10(sp)",
        "lw a1,  4 * 11(sp)",
        "lw a2,  4 * 12(sp)",
        "lw a3,  4 * 13(sp)",
        "lw a4,  4 * 14(sp)",
        "lw a5,  4 * 15(sp)",
        "lw a6,  4 * 16(sp)",
        "lw a7,  4 * 17(sp)",
        "lw s0,  4 * 18(sp)",
        "lw s1,  4 * 19(sp)",
        "lw s2,  4 * 20(sp)",
        "lw s3,  4 * 21(sp)",
        "lw s4,  4 * 22(sp)",
        "lw s5,  4 * 23(sp)",
        "lw s6,  4 * 24(sp)",
        "lw s7,  4 * 25(sp)",
        "lw s8,  4 * 26(sp)",
        "lw s9,  4 * 27(sp)",
        "lw s10, 4 * 28(sp)",
        "lw s11, 4 * 29(sp)",
        "lw sp,  4 * 30(sp)",
        "sret"
    )
}
```

Here are some key points:

- `sscratch` register is used as a temporary storage to save the stack pointer at the time of exception occurrence, which is later restored.
- Floating-point registers are not used within the kernel, and thus there's no need to save them here. Generally, they are saved and restored during thread switching.
- The stack pointer is set in the `a0` register, and the `handle_trap` function is called. At this point, the address pointed to by the stack pointer contains register values stored in the same structure as the `trap_frame` structure described later.
- Adding `.align 2` aligns the function's starting address to a 2<sup>2</sup> = 4-byte boundary. This is because the `stvec` register not only holds the address of the exception handler but also has flags representing the mode in its lower 2 bits.

> [!NOTE]
>
> The entry point of exception handlers is one of most critical and error-prone parts of the kernel. Reading the code closely, you'll notice that *original* values of general-purpose registers are saved onto the stack, even `sp` by using `sscratch`.
>
> If you accidentally overwrite `a0` register, it can lead to hard-to-debug problems like "local variable values change for no apparent reason". Save the program state perfectly not to spend your precious Saturday night debugging!

In the entry point, the following `handle_trap` function is called to handle the exception in our favorite Rust language:

```rust [kernel/src/entry.rs]
#[unsafe(no_mangle)]
extern "C" fn handle_trap(f: &mut TrapFrame) {
    let scause = read_csr!("scause");
    let stval = read_csr!("stval");
    let user_pc = read_csr!("sepc");

    panic!("unexpected trap scause=0x{:x}, stval=0x{:x}, sepc=0x{:x}", scause, stval, user_pc);
}
```

It reads why the exception has occurred, and triggers a kernel panic for debugging purposes. We use `no_mangle` so that our assembly can find this function by name, and `"C"` representation so that registers are used as function arguments in an expected way.

Let's define the various macros and data structures used here in `entry.rs`:

```rust [kernel/src/entry.rs]
#[repr(C, packed)]
struct TrapFrame{
    ra: usize,
    gp: usize,
    tp: usize,
    t0: usize,
    t1: usize,
    t2: usize,
    t3: usize,
    t4: usize,
    t5: usize,
    t6: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
    s0: usize,
    s1: usize,
    s2: usize,
    s3: usize,
    s4: usize,
    s5: usize,
    s6: usize,
    s7: usize,
    s8: usize,
    s9: usize,
    s10: usize,
    s11: usize,
    sp: usize,
}

#[allow(unused_imports)]
use crate::{read_csr, write_csr};

#[macro_export]
macro_rules! read_csr {
    ( $reg:literal ) => {
        {
            let val: usize;
            unsafe{core::arch::asm!(concat!("csrr {}, ", $reg), out(reg) val)}
            val
        }
    };
}

#[macro_export]
macro_rules! write_csr {
    ( $reg:literal, $val:expr ) => {
        {
            let val = $val; // Expand metavariable outside of unsafe block (avoids clippy warning)
            unsafe{core::arch::asm!(concat!("csrw ", $reg, ", {}"), in(reg) val)}
        }
    };
}
```

The `trap_frame` struct represents the program state saved in `kernel_entry`. We use a "C" representation so that the struct member order is kept as listed, and use `packed` to make sure no unwanted padding is added. `read_csr!` and `write_csr!` macros are convenient macros for reading and writing CSR registers.

The last thing we need to do is to tell the CPU where the exception handler is located. It's done by setting the `stvec` register in the `kernel_main` function:

```rust [kernel/src/main.rs] {2-5, 10-11}
...
#[macro_use]
mod entry;

use crate::entry::kernel_entry;
...
fn kernel_main() -> ! {
    ...

    write_csr!("stvec", kernel_entry as usize); // new
    unsafe{core::arch::asm!("unimp")}; // new

    panic!("booted!");
}
```

In addition to setting the `stvec` register, it executes `unimp` instruction. it's a pseudo instruction which triggers an illegal instruction exception.

> [!NOTE]
>
> **`unimp` is a "pseudo" instruction**.
>
> According to [RISC-V Assembly Programmer's Manual](https://github.com/riscv-non-isa/riscv-asm-manual/blob/main/src/asm-manual.adoc#instruction-aliases), the assembler translates `unimp` to the following instruction:
>
> ```
> csrrw x0, cycle, x0
> ```
>
> This reads and writes the `cycle` register into `x0`. Since `cycle` is a read-only register, CPU determines that the instruction is invalid and triggers an illegal instruction exception.

## Let's try it

Let's try running it and confirm that the exception handler is called:

```
$ ./run.sh
Hello World! 🦀
⚠️  Panic: panicked at kernel/src/entry.rs:128:5:
unexpected trap scause=0x2, stval=0x0, sepc=0x802002c2
```

According to the specification, when the value of `scause` is 2, it indicates an "Illegal instruction," meaning that program tried to execute an invalid instruction. This is precisely the expected behavior of the `unimp` instruction!

Let's also check where the value of `sepc` is pointing. If it's pointing to the line where the `unimp` instruction is called,  everything is working correctly:

```
$ addr2line -e kernel.elf 802002c2
~/src/os1k/kernel/src/main.rs:40
```
