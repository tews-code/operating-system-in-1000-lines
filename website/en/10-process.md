# Process

A process is an instance of an application. Each process has its own independent execution context and resources, such as a virtual address space.

> [!NOTE]
>
> Practical operating systems provide the execution context as a separate concept called a *"thread"*. For simplicity, in this book we'll treat each process as having a single thread.

## Process control block

The following `process` structure defines a process object. It's also known as  _"Process Control Block (PCB)"_.

Create a new module `process.rs`.

```rust [kernel/src/process.rs]
//! Process

use crate::address::VAddr;

const PROCS_MAX: usize = 8;    // Maximum number of processes

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum State {
    Unused,     // Unused process control structure
    Runnable,   // Runnable process
}

#[derive(Copy, Clone)]
pub struct Process {
    pub pid: usize,            // Process ID
    state: State,              // Process state: Unused or Runnable
    pub sp: VAddr,             // Stack pointer
    stack: [u8; 8192],         // Kernel stack
}

impl Process {
    const fn empty() -> Self {
        Self {
            pid: 0,
            state: State::Unused,
            sp: VAddr::new(0),
            stack: [0; 8192],
        }
    }
}
```
The kernel stack contains saved CPU registers, return addresses (where it was called from), and local variables. By preparing a kernel stack for each process, we can implement context switching by saving and restoring CPU registers, and switching the stack pointer.

> [!TIP]
>
> There is another approach called *"single kernel stack"*. Instead of having a kernel stack for each process (or thread), there's only single stack per CPU. [seL4 adopts this method](https://trustworthy.systems/publications/theses_public/05/Warton%3Abe.abstract).
>
> This *"where to store the program's context"* issue is also a topic discussed in async runtimes of programming languages like Go and Rust. Try searching for *"stackless async"* if you're interested.

## Context switch

Switching the process execution context is called *"context switching"*. The following `switch_context` function is the implementation of context switching:

```rust [kernel/src/process.rs]

use core::arch::naked_asm;

#[unsafe(naked)]
pub unsafe extern "C" fn switch_context(prev_sp: *mut usize, next_sp: *mut usize) {
    naked_asm!(
        ".align 2",
        // Save callee-saved registers onto the current process's stack.
        "addi sp, sp, -13 * 4", // Allocate stack space for 13 4-byte registers
        "sw ra,  0  * 4(sp)",  // Save callee-saved registers only
        "sw s0,  1  * 4(sp)",
        "sw s1,  2  * 4(sp)",
        "sw s2,  3  * 4(sp)",
        "sw s3,  4  * 4(sp)",
        "sw s4,  5  * 4(sp)",
        "sw s5,  6  * 4(sp)",
        "sw s6,  7  * 4(sp)",
        "sw s7,  8  * 4(sp)",
        "sw s8,  9  * 4(sp)",
        "sw s9,  10 * 4(sp)",
        "sw s10, 11 * 4(sp)",
        "sw s11, 12 * 4(sp)",

        // Switch the stack pointer.
        "sw sp, (a0)",         // *prev_sp = sp;
        "lw sp, (a1)",         // Switch stack pointer (sp) here

        // Restore callee-saved registers from the next process's stack.
        "lw ra,  0  * 4(sp)", // Restore callee-saved registers only
        "lw s0,  1  * 4(sp)",
        "lw s1,  2  * 4(sp)",
        "lw s2,  3  * 4(sp)",
        "lw s3,  4  * 4(sp)",
        "lw s4,  5  * 4(sp)",
        "lw s5,  6  * 4(sp)",
        "lw s6,  7  * 4(sp)",
        "lw s7,  8  * 4(sp)",
        "lw s8,  9  * 4(sp)",
        "lw s9,  10 * 4(sp)",
        "lw s10, 11 * 4(sp)",
        "lw s11, 12 * 4(sp)",
        "addi sp, sp, 13 * 4",  // We've popped 13 4-byte registers from the stack
        "ret",
    )
}
```

`switch_context` saves the callee-saved registers onto the stack, switches the stack pointer, and then restores the callee-saved registers from the stack. In other words, the execution context is stored as temporary local variables on the stack. Alternatively, you could save the context in `struct Process`, but this stack-based approach is beautifully simple, isn't it?

Callee-saved registers are registers that a called function must restore before returning. In RISC-V, `s0` to `s11` are callee-saved registers. Other registers like `a0` are caller-saved registers, and already saved on the stack by the caller. This is why `switch_context` handles only part of registers.

The `naked` attribute tells the compiler not to generate any other code than the inline assembly. It should work without this attribute, but it's a good practice to use it to avoid unintended behavior especially when you modify the stack pointer manually.

> [!TIP]
>
> Callee/Caller saved registers are defined in [Calling Convention](https://riscv.org/wp-content/uploads/2015/01/riscv-calling.pdf). Compilers generate code based on this convention.

Next, let's implement the process initialization function, `create_process`. It takes the entry point as a parameter, and returns the `pid` of the created `Process` struct.

First we create a static array of processes protected by a SpinLock called `PROCS`, and add some helper functions. Then we use `PROCS` when creating a new process:

```rust [kernel/src/process.rs]
use crate::spinlock::SpinLock;

pub struct Procs(pub SpinLock<[Process; PROCS_MAX]>);

impl Procs {
    const fn new() -> Self {
        Self(
            SpinLock::new([const { Process::empty() }; PROCS_MAX])
        )
    }

    pub fn try_get_index(&self, pid: usize) -> Option<usize> {
        self.0.lock().iter().position(|p| p.pid == pid)
    }
}

// Optional - but vital for debugging if you want to print the contents of PROCS.
// impl fmt::Display for Procs {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         let procs = PROCS.0.lock();
//         for (i, process) in procs.iter().enumerate() {
//             write!(f, "Addr: {:x?} ", &raw const *process as usize)?;
//             writeln!(f, "PROC[{i}]")?;
//             write!(f, "PID: {} ", process.pid)?;
//             write!(f, "SP: {:x?} ", process.sp)?;
//             writeln!(f, "STATE: {:?} ", process.state)?;
//             writeln!(f, "STACK: [ ... {:x?}]", &process.stack[8140..8191])?
//         }
//         Ok(())
//     }
// }

pub static PROCS: Procs = Procs::new();  // All process control structures.

pub fn create_process(pc: usize) -> usize {
    let mut procs = PROCS.0.lock();

    // Find an unused process control structure.
    let (i, process) = procs.iter_mut()
        .enumerate()
        .find(|(_, p)| p.state == State::Unused)
        .expect("no free process slots");

    // Stack callee-saved registers. These register values will be restored in
    // the first context switch in switch_context.
    let callee_saved_regs: [usize; 13] = [
        pc,            // ra
        0,             // s0
        0,             // s1
        0,             // s2
        0,             // s3
        0,             // s4
        0,             // s5
        0,             // s6
        0,             // s7
        0,             // s8
        0,             // s9
        0,             // s10
        0,             // s11
    ];

    // Place the callee-saved registers at the end of the stack
    let callee_saved_regs_start = process.stack.len() - callee_saved_regs.len() * size_of::<usize>();
    let mut offset = callee_saved_regs_start;
    for reg in &callee_saved_regs {
        let bytes = reg.to_ne_bytes(); // native endian
        process.stack[offset..offset + size_of::<usize>()].copy_from_slice(&bytes);
        offset += size_of::<usize>();
    }

    // Initialise fields.
    process.pid = i + 1;
    process.state = State::Runnable;
    process.sp = VAddr::new(&raw const process.stack[callee_saved_regs_start] as usize);

    process.pid
}
```

> [!TIP]
> The PROCS array is `static`, meaning it is placed in the `.bss` segment. When we call `context_switch`, our stack pointer will start to use the stack in the `.bss` segment directly. That's OK - any writeable memory can be used as a stack.
> You can see this in `kernel.map` by searching for "PROCS", which will be between `__bss` and `__bss_end`.

## Testing context switch

We have implemented the most basic function of processes - concurrent execution of multiple programs. Let's create two processes:

```rust [kernel/src/main.rs]
fn delay() {
    for _ in 0..300_000_000usize {
        unsafe{asm!("nop");} // do nothing
    }
}

static PROC_A: SpinLock<Option<usize>> = SpinLock::new(None);
static PROC_B: SpinLock<Option<usize>> = SpinLock::new(None);

fn proc_a_entry() {
    println!("starting process A");
    loop {
        print!("🐈");

        let proc_a_pid = PROC_A.lock().expect("should be initialised");
        let proc_b_pid = PROC_B.lock().expect("should be initialised");

        let (proc_a_sp_ptr, proc_b_sp_ptr) = PROCS
            .get_disjoint_sp_ptrs(proc_a_pid, proc_b_pid)
            .expect("failed to get stack pointers for context switch");

        unsafe {
            switch_context(
                proc_a_sp_ptr,
                proc_b_sp_ptr
            );
        }

        delay()
    }
}

fn proc_b_entry() {
    println!("starting process B");
    loop {
        print!("🐕");
        let proc_a_pid = PROC_A.lock().expect("should be initialised");
        let proc_b_pid = PROC_B.lock().expect("should be initialised");

        let (proc_a_sp_ptr, proc_b_sp_ptr) = PROCS
        .get_disjoint_sp_ptrs(proc_a_pid, proc_b_pid)
        .expect("failed to get stack pointers for context switch");

        unsafe {
            switch_context(
                proc_b_sp_ptr,
                proc_a_sp_ptr
            );
        }

        delay()
    }
}

#[unsafe(no_mangle)]
fn kernel_main() -> ! {
    let bss = &raw const __bss;
    let bss_end = &raw const __bss_end;
    // Safety: from linker script bss is aligned and bss segment is valid for writes up to bss_end
    unsafe {
        write_bytes(bss as *mut u8, 0, bss_end as usize - bss as usize);
    }

    write_csr!("stvec", kernel_entry as usize);

    common::println!("Hello World! 🦀");

    PROC_A.lock().get_or_insert_with(|| {
        create_process(proc_a_entry as usize)
    });
    PROC_B.lock().get_or_insert_with(|| {
        create_process(proc_b_entry as usize)
    });
    proc_a_entry();

    panic!("booted!");
}
```
The `proc_a_entry` function and `proc_b_entry` function are the entry points for Process A and Process B respectively. After displaying a single character using the `println!` macro, they switch context to the other process using the `switch_context` function.

`delay` function implements a busy wait to prevent the character output from becoming too fast, which would make your terminal unresponsive. `nop` instruction is a "do nothing" instruction. It is added to prevent compiler optimization from removing the loop.

Now, let's try! The startup messages will be displayed once each, and then "🐕🐈🐕🐈🐕..." lasts forever:

```
$ ./run.sh
Hello World! 🦀
starting process A
🐈starting process B
🐕🐈🐕🐈🐕🐈🐕🐈🐕🐈🐕🐈🐕🐈🐕🐈🐕🐈🐕🐈🐕🐈🐕🐈🐕🐈QE
```

## Scheduler

In the previous experiment, we directly called the `switch_context` function to specify the "next process to execute". However, this method becomes complicated when determining which process to switch to next as the number of processes increases. To solve the issue, let's implement a *"scheduler"*, a kernel program which decides the next process.

The following `yield_now` function is the implementation of the scheduler:

> [!TIP]
>
> The word "yield" is often used as the name for an API which allows giving up the CPU to another process voluntarily.

```rust [kernel/src/scheduler.rs]
//! Round-robin scheduler

use crate::process::{create_process, PROCS, PROCS_MAX, State, switch_context};
use crate::spinlock::SpinLock;
    
static IDLE_PROC: SpinLock<Option<usize>> = SpinLock::new(None);    // Idle process
pub static CURRENT_PROC: SpinLock<Option<usize>> = SpinLock::new(None); // Currently running process
const IDLE_PID: usize = 0; // idle

pub fn yield_now() {
    // Initialse IDLE_PROC if not yet initialised
    let idle_pid = { *IDLE_PROC.lock().get_or_insert_with(|| {
            let idle_pid = create_process(0);
            if let Some(p) = PROCS.0.lock().iter_mut()
                .find(|p| p.pid == idle_pid) {
                    p.pid = IDLE_PID;
                }
            *CURRENT_PROC.lock() = Some(IDLE_PID);
            IDLE_PID
        })
    };

    let current_pid = CURRENT_PROC.lock()
        .expect("CURRENT_PROC initialised before use");

    // Search for a runnable process
    let next_pid = {
        let current_index = PROCS.try_get_index(current_pid)
            .expect("current process PID should have an index");
        PROCS.0.lock().iter()
            .cycle()
            .skip(current_index + 1)
            .take(PROCS_MAX)
            .find(|p| p.state == State::Runnable && p.pid != idle_pid)
            .map(|p| p.pid)
            .unwrap_or(idle_pid)
    };

    // If there's no runnable process other than the current one, return and continue processing
    if next_pid == current_pid {
        return;
    }

    // Get current and next SP pointers from the PROCS array at the same time
    let (next_sp_ptr, current_sp_ptr) = {
        let next_index = PROCS.try_get_index(next_pid)
            .expect("should find next by pid");
        let current_index = PROCS.try_get_index(current_pid)
            .expect("should find current by pid");
        let mut procs = PROCS.0.lock();
        let [next, current] = procs.get_disjoint_mut([next_index, current_index])
            .expect("indices should be valid and distinct");

        let next_sp_ptr = next.sp.field_raw_ptr();
        let current_sp_ptr = current.sp.field_raw_ptr();

        (next_sp_ptr, current_sp_ptr)
    };

    // Context switch
    *CURRENT_PROC.lock() = Some(next_pid);
    unsafe {
        switch_context(current_sp_ptr, next_sp_ptr);
    }
}
```
Here, we introduce two global variables. `CURRENT_PROC` points to the currently running process. `IDLE_PROC` refers to the idle process, which is "the process to run when there are no runnable processes". The `IDLE_PROC` is created on initialisation as a process with process ID `0`. 

The key point of this initialization process is `*CURRENT_PROC.lock() = Some(IDLE_PID);`. This ensures that the execution context of the boot process is saved and restored as that of the idle process. During the first call to the `yield_now` function, it switches from the idle process to process A, and when switching back to the idle process, it behaves as if returning from this `yield_now` function call.

Update `main.rs` to make use of our new scheduler. Modify `proc_a_entry` and `proc_b_entry` as follows to call the `yield_now` function instead of directly calling the `switch_context` function:
```rust [kernel/src/main.rs]
...
mod scheduler;
...
use crate::process::create_process;
use crate::scheduler::yield_now;
...
static PROC_A: SpinLock<Option<usize>> = SpinLock::new(None);
static PROC_B: SpinLock<Option<usize>> = SpinLock::new(None);

fn proc_a_entry() {
    println!("starting process A");
    loop {
        print!("🐈");
        yield_now();
        delay()
    }
}

fn proc_b_entry() {
    println!("starting process B");
    loop {
        print!("🐕");
        yield_now();
        delay()
    }
}


#[unsafe(no_mangle)]
fn kernel_main() -> ! {
    let bss = &raw const __bss;
    let bss_end = &raw const __bss_end;
    // Safety: from linker script bss is aligned and bss segment is valid for writes up to bss_end
    unsafe {
        write_bytes(bss as *mut u8, 0, bss_end as usize - bss as usize);
    }

    write_csr!("stvec", kernel_entry as usize);

    common::println!("Hello World! 🦀");

    PROC_A.lock().get_or_insert_with(|| {
        create_process(proc_a_entry as usize)
    });
    PROC_B.lock().get_or_insert_with(|| {
        create_process(proc_b_entry as usize)
    });

    yield_now();

    panic!("switched to idle process");
}
...
```

If "🐈" and "🐕" are printed as before, it works perfectly!

## Changes in the exception handler

In the exception handler, it saves the execution state onto the stack. However, since we now use separate kernel stacks for each process, we need to update it slightly.

First, store a pointer to the bottom of the kernel stack for the currently executing process in the `sscratch` register during process switching.
We will read this during the exception handler (see [Appendix: Why do we reset the stack pointer?](#appendix-why-do-we-reset-the-stack-pointer) for more explanation).

(We do this using the `asm!` macro, instead of our own `write_csr!` macro as we will be expanding this assembly in the next chapter.)

```rust [kernel/src/scheduler.rs] {2, 7, 18, 19, 22-25}
...
use core::arch::asm;
...
/* Omitted */
pub fn yield_now() {
    ...
    let (next_sp_ptr, current_sp_ptr, sscratch) = {
        let next_index = PROCS.try_get_index(next_pid)
            .expect("should find next by pid");
        let current_index = PROCS.try_get_index(current_pid)
            .expect("should find current by pid");
        let mut procs = PROCS.0.lock();
        let [next, current] = procs.get_disjoint_mut([next_index, current_index])
            .expect("indices should be valid and distinct");

        let next_sp_ptr = next.sp.field_raw_ptr();
        let current_sp_ptr = current.sp.field_raw_ptr();
        let sscratch = unsafe { next.stack.as_ptr().add(next.stack.len()) };
        (next_sp_ptr, current_sp_ptr, sscratch)
    };

    unsafe{asm!(
        "csrw sscratch, {sscratch}",
        sscratch = in(reg) sscratch,
    )};

    // Context switch
    *CURRENT_PROC.lock() = Some(next_pid);
    let (current_sp_ptr, next_sp_ptr) = PROCS
        .get_disjoint_sp_ptrs(current_pid, next_pid)
        .expect("failed to get stack pointers for context switch");
    unsafe {
        switch_context(current_sp_ptr, next_sp_ptr);
    }
}
```
Since the stack pointer extends towards lower addresses, we set the address at one past the end of the stack as the initial value of the kernel stack.

The modifications to the exception handler are as follows:

```rust [kernel/src/entry.rs]
#[unsafe(naked)]
pub unsafe extern "C" fn kernel_entry() {
    naked_asm!(
        ".align 2",
        // Retrieve the kernel stack of the running process from sscratch.
        "csrrw sp, sscratch, sp",
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

        // Retrieve and save the sp at the time of exeception
        "csrr a0, sscratch",
        "sw a0, 4 * 30(sp)",

        // Reset the kernel stack.
        "addi a0, sp, 4 * 31",
        "csrw sscratch, a0",

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

The first `csrrw` instruction is a swap operation in short:

```
tmp = sp;
sp = sscratch;
sscratch = tmp;
```

Thus, `sp` now points to the *kernel* (not *user*) stack of the currently running process. Also, `sscratch` now holds the original value of `sp` (user stack) at the time of the exception.

After saving other registers onto the kernel stack, we restore the original value of `sp` from `sscratch` and save it onto the kernel stack.
Then, we calculate the original value that `sscratch` had when the exception handler was called and restore it.

> [!NOTE]
>
> We are overwriting the bottom 31 words of the kernel stack when we save these registers: our simple OS does not support nested interrupt handling.
> When the CPU enters the `stvec` handler (`kernel_entry`), it automatically disables interrupts until it returns to user mode, and anyway our kernel panics when an exception occurs.
> One possible approach for handling nested interrupts is to have a separate `stvec` handler for user mode and for kernel mode.

The key point here is that each process has its own independent kernel stack. By switching the contents of `sscratch` during context switching, we can resume the execution of the process from the point where it was interrupted, as if nothing had happened.

> [!TIP]
>
> We've implemented the context switching mechanism for the "kernel" stack. The stack used by applications (so-called *user stack*) will be allocated separately from the kernel stack. This will be implemented in later chapters.

## Appendix: Why do we reset the stack pointer?

In the previous section, you might have wondered why we need to switch to the kernel stack by tweaking `sscratch`.

This is because we must not trust the stack pointer at the time of exception. In the exception handler, we need to consider the following three patterns:

1. An exception occurred in kernel mode.
2. An exception occurred in kernel mode, when handling another exception (nested exception).
3. An exception occurred in user mode.

In case (1), there's generally no problem even if we don't reset the stack pointer. In case (2), we would overwrite the saved area, but our implementation triggers a kernel panic on nested exceptions, so it's OK.

The problem is with case (3). In this case, `sp` points to the "user (application) stack area". If we implement it to use (trust) `sp` as is, it could lead to a vulnerability that crashes the kernel.

Let's experiment with this by running the following application after completing all the implementations up to Chapter 17 in this book:

```c
// An example of applications
#include "user.h"

void main(void) {
    __asm__ __volatile__(
        "li sp, 0xdeadbeef\n"  // Set an invalid address to sp
        "unimp"                // Trigger an exception
    );
}
```

If we run this without applying the modifications from this chapter (i.e. restoring the kernel stack from `sscratch`), the kernel hangs without displaying anything, and you'll see the following output in QEMU's log:

```
epc:0x0100004e, tval:0x00000000, desc=illegal_instruction <- unimp triggers the trap handler
epc:0x802009dc, tval:0xdeadbe73, desc=store_page_fault <- an aborted write to the stack  (0xdeadbeef)
epc:0x802009dc, tval:0xdeadbdf7, desc=store_page_fault <- an aborted write to the stack  (0xdeadbeef) (2)
epc:0x802009dc, tval:0xdeadbd7b, desc=store_page_fault <- an aborted write to the stack  (0xdeadbeef) (3)
epc:0x802009dc, tval:0xdeadbcff, desc=store_page_fault <- an aborted write to the stack  (0xdeadbeef) (4)
...
```

First, an invalid instruction exception occurs with the `unimp` pseudo-instruction, transitioning to the kernel's trap handler. However, because the stack pointer points to an unmapped address (`0xdeadbeef`), an exception occurs when trying to save registers, jumping back to the beginning of the trap handler. This becomes an infinite loop, causing the kernel to hang. To prevent this, we need to retrieve a trusted stack area from `sscratch`.

Another solution is to have multiple exception handlers. In the RISC-V version of xv6 (a famous educational UNIX-like OS), there are separate exception handlers for cases (1) and (2) ([`kernelvec`](https://github.com/mit-pdos/xv6-riscv/blob/f5b93ef12f7159f74f80f94729ee4faabe42c360/kernel/kernelvec.S#L13-L14)) and for case (3) ([`uservec`](https://github.com/mit-pdos/xv6-riscv/blob/f5b93ef12f7159f74f80f94729ee4faabe42c360/kernel/trampoline.S#L74-L75)). In the former case, it inherits the stack pointer at the time of the exception, and in the latter case, it retrieves a separate kernel stack. The trap handler is [switched](https://github.com/mit-pdos/xv6-riscv/blob/f5b93ef12f7159f74f80f94729ee4faabe42c360/kernel/trap.c#L44-L46) when entering and exiting the kernel.

> [!TIP]
>
> In Fuchsia, an OS developed by Google, there was a case where an API allowing arbitrary program counter values to be set from the user became [a vulnerability](https://blog.quarkslab.com/playing-around-with-the-fuchsia-operating-system.html). Not trusting input from users (applications) is an extremely important habit in kernel development.

## Next Steps

We have now achieved the ability to run multiple processes concurrently, realizing a multi-tasking OS.

However, as it stands, processes can freely read and write to the kernel's memory space. It's super insecure! In the coming chapters, we'll look at how to safely run applications, in other words, how to isolate the kernel and applications.
