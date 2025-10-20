# Kernel Panic

A kernel panic occurs when the kernel encounters an unrecoverable error, similar to the concept of `panic` in Go. Have you ever seen a blue screen on Windows? Let's implement the same concept in our kernel to handle fatal errors.

Create a new module file `panic.rs`, move the panic handler in this file, and add the module to `main.rs` 

The following panic handler is the implementation of kernel panic using println!:

```rust [kernel/src/panic.rs]
//! Panic for os1k

use core::arch::asm;
use core::panic::PanicInfo;

use crate::println;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("⚠️ Panic: {}", info);

    loop {
        unsafe {asm!("wfi")};
    }
}
```

It prints where the panic occurred, it enters an infinite loop to halt processing. In our loop use the wait for interrupt `wfi` command, which allows the processor to move to a low power state while waiting for an interrupt (which never comes).

## Let's try it

Let's try using `panic!`. You can use it like `println!`:

```rust [kernel/src/main.rs]
#[unsafe(no_mangle)]
fn kernel_main() -> ! {
    let bss = &raw const __bss;
    let bss_end = &raw const __bss_end;
    // Safety: from linker script bss is aligned and bss segment is valid for writes up to bss_end
    unsafe {
        write_bytes(bss as *mut u8, 0, bss_end as usize - bss as usize);
    }

    panic!("booted!");
}
```

Try in QEMU and confirm that the correct file name and line number are displayed, and that any processing after `panic!` is not executed (i.e., `println!("unreachable here!");` causes compiler warnings and is not displayed).

```
$ ./run.sh
⚠️ Panic: panicked at kernel/src/main.rs:33:5:
booted!
```

Blue screen in Windows and kernel panics in Linux are very scary, but in your own kernel, don't you think it is a nice feature to have? It's a "crash gracefully" mechanism, with a human-readable clue.
