# Memory Allocation

In this chapter, we'll implement a simple memory allocator.

## Revisiting the linker script

Before implementing a memory allocator, let's define the memory regions to be managed by the allocator:

```ld [kernel.ld] {5-8}
    . = ALIGN(4);
    . += 128 * 1024; /* 128KB */
    __stack_top = .;

    . = ALIGN(4096);
    __free_ram = .;
    . += 64 * 1024 * 1024; /* 64MB */
    __free_ram_end = .;
}
```

This adds two new symbols: `__free_ram` and `__free_ram_end`. This defines a memory area after the stack space. The size of the space (64MB) is an arbitrary value and `. = ALIGN(4096)` ensures that it's aligned to a 4KB boundary.

By defining this in the linker script instead of hardcoding addresses, the linker can determine the position to avoid overlapping with the kernel's static data.

> [!TIP]
>
> Practical operating systems on x86-64 determine available memory regions by obtaining information from hardware at boot time (for example, UEFI's `GetMemoryMap`).

## The world's simplest memory allocation algorithm

Let's implement a function to allocate memory dynamically. Instead of allocating in bytes like `malloc`, it allocates in a larger unit called *"pages"*. 1 page is typically 4KB (4096 bytes).

> [!TIP]
>
> 4KB = 4096 = 0x1000 (hexadecimal). Thus, page-aligned addresses look nicely aligned in hexadecimal.

The following allocator function dynamically allocates pages of memory. Create a new module file `allocator.rs` and add the module in `main.rs`.

```rust [kernel/src/allocator.rs]
//! Allocate memory pages

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::write_bytes;

use crate::address::{align_up, PAddr};
use crate::spinlock::SpinLock;

const PAGE_SIZE: usize = 4096;

//Safety: Symbols created by linker script
unsafe extern "C" {
    static __free_ram: u8;
    static __free_ram_end: u8;
}

#[derive(Debug)]
struct BumpAllocator(SpinLock<Option<PAddr>>);

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator(
    SpinLock::new(None),
);

unsafe impl GlobalAlloc for BumpAllocator {
    // Safety: Caller must ensure that Layout has a non-zero size
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        debug_assert!(layout.size() > 0, "allocation size must be non-zero");

        let mut next_paddr = self.0.lock();

        // Initialise on first use
        if next_paddr.is_none() {
            *next_paddr = Some(PAddr::new(&raw const __free_ram as usize))
        }

        // Safe to unwrap as we know it is Some now
        let paddr = next_paddr.unwrap();
        let new_paddr = paddr.as_usize() + align_up(layout.size(), PAGE_SIZE);
        if new_paddr > &raw const __free_ram_end as usize {
            panic!("out of memory");
        }
        *next_paddr = Some(PAddr::new(new_paddr));

        // Safety: paddr.as_ptr() is aligned and not null; entire PAGE_SIZE of bytes is available for write
        unsafe{ write_bytes(paddr.as_ptr() as *mut usize, 0, PAGE_SIZE) };

        // Testing only - comment out
        common::println!("alloc_pages test: {:x}", paddr.as_usize());

        paddr.as_ptr() as *mut u8
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}
```

We are going to use the Rust `GlobalAllocator` trait used for building allocators. We create a struct `BumpAllocator` to associate with the trait, and in our simple allocator all we need is the next physical address to allocate. We then need to provide two functions for the trait: `alloc` and `dealloc` - and in return we can use the `alloc` crate.

The `alloc` function signature is ` unsafe fn alloc(&self, layout: Layout) -> *mut u8`, where the allocation request is provided as a `Layout` type: 

```rust
pub struct Layout {
    size: usize,
    align: Alignment,
}
```
To simplify things, we will ignore the requested alignment, and always align to a full page.

You will find the following key points:

- `ALLOCATOR` is defined as a `static` variable. This means, unlike local variables, its value is retained between function calls. That is, it behaves like a global variable. However, we need to change (mutate) the value despite it being static, so we need "interior mutability", which we get from `SpinLock`. At first the allocator is uninitialised, so we use `Option` to return `None` until initialisation is complete. The resulting definition of `ALLOCATOR` is a `struct BumpAllocator(SpinLock<Option<PAddr>>);`
- PAGE_SIZE` represents the size of one page.
- `next_paddr` points to the start address of the "next area to be allocated" (free area). When allocating, `next_paddr` is advanced by the size being allocated.
- `next_paddr` initially holds the address of `__free_ram`. This means memory is allocated sequentially starting from `__free_ram`.
- `__free_ram` is placed on a 4KB boundary due to `ALIGN(4096)` in the linker script. Therefore, the `alloc` function always returns an address aligned to 4KB.
- If it tries to allocate beyond `__free_ram_end`, in other words, if it runs out of memory, a kernel panic occurs.
- The `write_bytes` function ensures that the allocated memory area is always filled with zeroes. This is to avoid hard-to-debug issues caused by uninitialized memory.
- The `dealloc` function does nothing - we never deallocate memory.

Isn't it simple? However, there is a big problem with this memory allocation algorithm: allocated memory cannot be freed! That said, it's good enough for our simple hobby OS.

> [!TIP]
>
> This algorithm is known as **Bump allocator** or **Linear allocator**, and it's actually used in scenarios where deallocation is not necessary. It's an attractive allocation algorithm that can be implemented in just a few lines and is very fast.
>
> When implementing deallocation, it's common to use a bitmap-based algorithm or use an algorithm called the buddy system.

## Let's try memory allocation

Let's test the memory allocation function we've implemented. Add some code to `kernel_main`:

```rust [kernel/src/main.rs]
...
pub extern crate alloc;
use alloc::{string::String, vec, boxed::Box};
...
fn kernel_main {
    ...
    let s = String::from("Hello World! 🦀");
    let v = vec![1, 2, 3];
    let b = Box::new(42);
    
    println!("We can now allocate! Let's try strings: {s}, vectors: {v:?} and boxes: {b}");

    panic!("booted";
}

Verify that the first address matches the address of `__free_ram`, and that the final address matches an address 8KB after:

```
$ ./run.sh
alloc_pages test: 80222000
alloc_pages test: 80223000
alloc_pages test: 80224000
We can now allocate! Let's try strings: Hello World! 🦀, vectors: [1, 2, 3] and boxes: 42
```

```
$ llvm-nm kernel.elf | grep __free_ram
80222000 B __free_ram
84222000 B __free_ram_end
```
