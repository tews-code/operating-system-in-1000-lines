# User Mode

In this chapter, we'll run the application we created in the previous chapter.

## Extracting the executable file

In executable file formats like ELF, the load address is stored in its file header (program header in ELF). However, since our application's execution image is a raw binary, we need to prepare it with a fixed value. 

We will create a process for the application, so we add this to `kernel/src/entry.rs` like this:

```rust [kernel/src/entry.rs]
// The base virtual address of an application image. This needs to match the
// starting address defined in `user.ld`.
pub const USER_BASE: usize = 0x1000000;
```

Also, update the `create_process` function to start the application:

```rust [kernel/src/entry.rs]
fn user_entry() {
    to_do!();
}
```

In `kernel/src/process.rs`, we are going to update the `create_process` function to create a process from a user image:

```rust [kernel/src/process.rs]
pub fn create_process(image: *const u8, image_size: usize) -> usize {
    ...
    let callee_saved_regs: [usize; 13] = [
        user_entry as usize,            // ra
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

        // Map kernel pages.
    let mut page_table = Box::new(PageTable::new());
    let kernel_base = &raw const __kernel_base as usize;
    let free_ram_end = &raw const __free_ram_end as usize;

    for paddr in (kernel_base..free_ram_end).step_by(PAGE_SIZE) {
        map_page(page_table.as_mut(), VAddr::new(paddr), PAddr::new(paddr), PAGE_R | PAGE_W | PAGE_X);
    }

    process.page_table = Some(page_table);

    // Map user pages.
    let aligned_size = align_up(image_size, PAGE_SIZE);
    let image_slice = unsafe {
        slice::from_raw_parts(image, image_size)
    };
    let mut image_vec = image_slice.to_vec();
    image_vec.resize(aligned_size, 0);
    let image_data = Box::leak(image_vec.into_boxed_slice());
    let page_table = process.page_table.as_mut()
    .expect("page table must be initialized before mapping user pages");

    for (i, page_chunk) in image_data.chunks_mut(PAGE_SIZE).enumerate() {
        let vaddr = VAddr::new(USER_BASE + i * PAGE_SIZE);
        let paddr = PAddr::new(page_chunk.as_mut_ptr() as usize);

        map_page(
            page_table,
            vaddr,
            paddr,
            PAGE_U | PAGE_R | PAGE_W | PAGE_X,
        );
    }
```
We've modified `create_process` to take the pointer to the execution image (`image`) and the image size (`image_size`) as arguments. It copies the execution image page by page for the specified size and maps it to the process' page table. Also, it sets the jump destination for the first context switch to `user_entry`. For now, we'll keep this as an empty function.

> [!WARNING]
>
> If you map the execution image directly without copying it, processes of the same application would end up sharing the same physical pages. It ruins the memory isolation!


Lastly, modify the caller of the `create_process` function and make it create a user process:

```rust [kernel/src/main.rs]
unsafe extern "C" {
    static _binary_shell_bin_start: u8;
    static _binary_shell_bin_size: u8;
}

#[unsafe(no_mangle)]
fn kernel_main() -> ! {
    ...

    // new!
    let shell_start = &raw const _binary_shell_bin_start as *mut u8;
    let shell_size = &raw const _binary_shell_bin_size as usize;  // The symbol _address_ is the size of the binary
    let _ = create_process(shell_start, shell_size);

    yield_now();

    panic!("switched to idle process");
}
```

Let's try it and check with the QEMU monitor if the execution image is mapped as expected:

```
(qemu) info mem
vaddr    paddr            size     attr
-------- ---------------- -------- -------
01000000 0000000080287000 00001000 rwxu-a-
01001000 0000000080288000 00010000 rwxu---
80200000 0000000080200000 00001000 rwx--a-
80201000 0000000080201000 00011000 rwx----
80212000 0000000080212000 00001000 rwx--a-
...
```

We can see that the physical address `0x80287000` is mapped to the virtual address `0x1000000` (`USER_BASE`). Let's take a look at the contents of this physical address. To display the contents of physical memory, use `xp` command:

```
(qemu) xp /32b 0x80287000
0000000080287000: 0x17 0x01 0x01 0x00 0x13 0x01 0x01 0x02
0000000080287008: 0x97 0x00 0x00 0x00 0xe7 0x80 0x00 0x01
0000000080287010: 0x97 0x00 0x00 0x00 0xe7 0x80 0xa0 0x00
0000000080287018: 0x01 0xa0 0x01 0xa0 0x00 0x00 0x00 0x00
```

It seems some data is present. Check the contents of `shell.bin` to confirm that it indeed matches:

```
$ hexdump -C shell.bin | head
00000000  17 01 01 00 13 01 01 02  97 00 00 00 e7 80 00 01  |................|
00000010  97 00 00 00 e7 80 a0 00  01 a0 01 a0 00 00 00 00  |................|
00000020  00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00  |................|
...
```

Hmm, it's hard to understand in hexadecimal. Let's disassemble the machine code to see if it matches the expected instructions:

```
(qemu) xp /8i 0x80287000
0x80287000:  00010117          auipc                   sp,16                   # 0x80297000
0x80287004:  02010113          addi                    sp,sp,32
0x80287008:  00000097          auipc                   ra,0                    # 0x80287008
0x8028700c:  010080e7          jalr                    ra,ra,16
0x80287010:  00000097          auipc                   ra,0                    # 0x80287010
0x80287014:  00a080e7          jalr                    ra,ra,10
0x80287018:  a001              j                       0                       # 0x80287018
0x8028701a:  a001              j                       0                       # 0x8028701a
```

It calculates/fills the initial stack pointer value, and then calls two different functions. If we compare this with the disassembly results of `shell.elf`, we can confirm that it indeed matches:

```
$ llvm-objdump -d target/riscv32imac-unknown-none-elf/debug/shell | head -n20

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

## Transition to user mode

To run applications, we use a CPU mode called *user mode*, or in RISC-V terms, *U-Mode*. It's surprisingly simple to switch to U-Mode. Here's how:

```rust [kernel/src/entry.rs]
...
// The base virtual address of an application image. This needs to match the
// starting address defined in `user.ld`.
pub const USER_BASE: usize = 0x1000000;
const SSTATUS_SPIE: usize =  1 << 5;    // Enable user mode

#[unsafe(naked)]
pub extern "C" fn  user_entry() {
    naked_asm!(
        "li t0, {user_base}",
        "csrw sepc, t0",
        "li t0, {sstatus}",
        "csrw sstatus, t0",
        "sret",
        user_base = const USER_BASE,
        sstatus = const SSTATUS_SPIE,
    )
}
...
```

The switch from S-Mode to U-Mode is done with the `sret` instruction. However, before changing the operation mode, it does two writes to CSRs:

> [!NOTE]
>
> To be precise, the `sret` instruction transitions to the user mode if the SPP bit in `sstatus` is 0. See [12.1.1. Supervisor Status Register (`sstatus`)](https://riscv.github.io/riscv-isa-manual/snapshot/privileged/#sstatus:~:text=When%20an%20SRET%20instruction%20(see%20Section%203.3.2)%20is%20executed%20to%20return%20from%20the%20trap%20handler%2C%20the%20privilege%20level%20is%20set%20to%20user%20mode%20if%20the%20SPP%20bit%20is%200) in the RISC-V spec for more details.

- Set the program counter for when transitioning to U-Mode in the `sepc` register. That is, where `sret` jumps to.
- Set the `SPIE` bit in the `sstatus` register. Setting this enables hardware interrupts when entering U-Mode, and the handler set in the `stvec` register will be called.

> [!TIP]
>
> In this book, we don't use hardware interrupts but use polling instead, so it's not necessary to set the `SPIE` bit. However, it's better to be clear rather than silently ignoring interrupts.

## Try user mode

Now let's try it! That said, because `shell.c` just loops infinitely, we can't tell if it's working properly on the screen. Instead, let's take a look with the QEMU monitor:

```
(qemu) info registers

CPU#0
 V      =   0
 pc       01000018
```

It seems CPU is continuously executing `0x01000018`. It appears to be working properly, but somehow it doesn't feel satisfying. So, let's see if we can observe behavior which is specific to U-Mode. Add these lines to `shell.rs`:

```rust [user/src/bin/shell.rs]
fn main() -> ! {
    unsafe {
        let ptr = 0x80200000 as *mut i32;
        core::ptr::write_volatile(ptr, 0x1234);
    }
    
    loop {}
}
```

This `0x80200000` is a memory area used by the kernel that is mapped on the page table. However, since it is a kernel page where the `U` bit in the page table entry is not set, an exception (page fault) should occur, and the kernel should panic. Let's try it:

```
$ ./os1k.sh

⚠️ Panic: panicked at kernel/src/entry.rs:133:5:
unexpected trap scause=0xf, stval=0x80200000, sepc=0x1000022

```

The 15th exception (`scause = 0xf = 15`), it corresponds to "Store/AMO page fault". It seems the expected exception happened!

Congrats! You've successfully executed your first application! Isn't it surprising how easy it is to implement user mode? The kernel is very similar to an application - it just has a few more privileges.
