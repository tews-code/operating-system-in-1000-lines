# Page Table

## Memory management and virtual addressing

When a program accesses memory, CPU translates the specified address (*virtual* address) into a physical address. The table that maps virtual addresses to physical addresses is called a *page table*. By switching page tables, the same virtual address can point to different physical addresses. This allows isolation of memory spaces (virtual address spaces) and separation of kernel and application memory areas, enhancing system security.

In this chapter, we'll implement the hardware-based memory isolation mechanism.

## Structure of virtual address

In this book, we use one of RISC-V's paging mechanisms called Sv32, which uses a two-level page table. The 32-bit virtual address is divided into a first-level page table index (`VPN[1]`), a second-level index (`VPN[0]`), and a page offset.

Try **[RISC-V Sv-32 Virtual Address Breakdown](https://riscv-sv32-virtual-address.vercel.app/)** to see how virtual addresses are broken down into page table indices and offsets.

Here are some examples:

| Virtual Address | `VPN[1]` (10 bits) | `VPN[0]` (10 bits) | Offset (12 bits) |
| --------------- | ------------------ | ------------------ | ---------------- |
| 0x1000_0000     | 0x040              | 0x000              | 0x000            |
| 0x1000_1000     | 0x040              | 0x001              | 0x000            |
| 0x1000_f000     | 0x040              | 0x00f              | 0x000            |
| 0x2000_f0ab     | 0x080              | 0x00f              | 0x0ab            |
| 0x2000_f012     | 0x080              | 0x00f              | 0x012            |
| 0x2000_f034     | 0x080              | 0x00f              | 0x034            |
| 0x20f0_f034     | 0x083              | 0x30f              | 0x034            |

> [!TIP]
>
> From the examples above, we can see the following characteristics of the indices:
>
> - Changing the middle bits (`VPN[0]`) doesn't affect the first-level index. This means page table entries for nearby addresses are concentrated in the same first-level page table.
> - Changing the lower bits doesn't affect either `VPN[1]` or `VPN[0]`. This means addresses within the same 4KB page are in the same page table entry.
>
> This structure utilizes [the principle of locality](https://en.wikipedia.org/wiki/Locality_of_reference), allowing for smaller page table sizes and more effective use of the Translation Lookaside Buffer (TLB).

When accessing memory, CPU calculates `VPN[1]` and `VPN[0]` to identify the corresponding page table entry, reads the mapped base physical address, and adds `offset` to get the final physical address.

## Constructing the page table

Let's construct a page table in Sv32. First, we'll create a new file `page.rs` and define some constants. `SATP_SV32` is a single bit in the `satp` register which indicates "enable paging in Sv32 mode", and `PAGE_*` are flags to be set in page table entries.

```rust [kernel/src/page.rs]
pub const SATP_SV32: usize = 1 << 31;
pub const PAGE_V: usize = 1 << 0;   // "Valid" bit (entry is enabled)
pub const PAGE_R: usize = 1 << 1;   // Readable
pub const PAGE_W: usize = 1 << 2;   // Writable
pub const PAGE_X: usize = 1 << 3;   // Executable
pub const PAGE_U: usize = 1 << 4;   // User (accessible in user mode)
```

Let's also extend our definitions of `VAddr` and `PAddr` to with helper methods to convert to and from `vpn` and `ppn` values:

```rust [kernel/src/page.rs]
...
use crate::allocator::PAGE_SIZE;

impl VAddr {
    fn vpn0(&self) -> usize {
        self.as_usize() >> 12 & 0x3FF
    }

    fn vpn1(&self) -> usize {
        self.as_usize() >> 22 & 0x3FF
    }
}

impl PAddr {
    fn ppn(&self) -> usize {
        (self.as_usize() / PAGE_SIZE) << 10
    }

    fn from_ppn(pte: usize) -> Self {
        PAddr::new((pte >> 10) * PAGE_SIZE)
    }
}
```

Now define a structure representing a page table: 

```rust [kernel/src/page.rs]
...
const ENTRIES_PER_TABLE: usize = 1024; // Each Page Table Entry is 4 bytes in Sv32

#[derive(Copy, Clone, Debug)]
pub struct PageTable([usize; ENTRIES_PER_TABLE]);

impl PageTable {
    pub const fn new() -> Self {
        Self([0; ENTRIES_PER_TABLE])
    }
}

impl Index<usize> for PageTable {
    type Output = usize;

    fn index(&self, vpn: usize) -> &Self::Output {
        &self.0[vpn]
    }
}

impl IndexMut<usize> for PageTable {
    fn index_mut(&mut self, vpn: usize) -> &mut Self::Output {
        &mut self.0[vpn]
    }
}
```

Here we define the page table as an array of 1024 page table entries. As we want to index into this table using page numbers, we add the traits `Index` and `IndexMut`. 

## Mapping pages

The following `map_page` function takes the first-level page table (`table1`), the virtual address (`vaddr`), the physical address (`paddr`), and page table entry flags (`flags`):

```rust [kernel/src/page.rs]
pub fn map_page(table1: &mut PageTable, vaddr: VAddr, paddr: PAddr, flags: usize) {
    assert!(is_aligned(vaddr.as_usize(), PAGE_SIZE), "unaligned vaddr {}", vaddr.as_usize());

    assert!(is_aligned(paddr.as_usize(), PAGE_SIZE), "unaligned paddr {}", paddr.as_usize());

    let vpn1 = vaddr.vpn1();

    // Create the 1st level page table if it doesn't exist.
    if table1[vpn1] & PAGE_V == 0 {
        let table0 = Box::new(PageTable::new());
        let table0_paddr = PAddr::new(Box::into_raw(table0) as *mut _ as usize);
        table1[vpn1] = table0_paddr.ppn() | PAGE_V;
    }

    let table0 = unsafe {
        let mut table0_paddr = PAddr::from_ppn(table1[vpn1]);
        &mut *(table0_paddr.as_ptr_mut() as *mut PageTable)
    };

    table0[vaddr.vpn0()] = paddr.ppn() | flags | PAGE_V;
}
```

This function ensures the first-level page table entry exists (creating the second-level table if needed), and then sets the second-level page table entry to map the physical page.

It divides `paddr` by `PAGE_SIZE` because the entry should contain the physical page number, not the physical address itself.

> [!IMPORTANT]
>
> Physical addresses and physical page numbers (PPN) are different. Be careful not to confuse them when setting up a page table.

## Mapping kernel memory area

The page table must be configured not only for applications (user space), but also for the kernel.

In this book, the kernel memory mapping is configured so that the kernel's virtual addresses match the physical addresses (i.e. `vaddr == paddr`). This allows the same code to continue running even after enabling paging.

First, let's modify the kernel's linker script to define the starting address used by the kernel (`__kernel_base`):

```ld [kernel.ld] {5}
ENTRY(boot)

SECTIONS {
    . = 0x80200000;
    __kernel_base = .;
```

> [!WARNING]
>
> Define `__kernel_base` **after** the line `. = 0x80200000`. If the order is reversed, the value of `__kernel_base` will be zero.

Next, add the page table to the process struct. This will be a pointer to the first-level page table.

```rust [kernel/src/process.rs] {2-4, 10, 20}
...
use crate::address::{PAddr, VAddr};
use crate::allocator::PAGE_SIZE;
use crate::page::{map_page, PageTable, PAGE_R, PAGE_W, PAGE_X};
...
pub struct Process {
    pub pid: usize,            // Process ID
    pub state: State,          // Process state: Unused or Runnable
    pub sp: VAddr,             // Stack pointer
    pub page_table: Option<Box<PageTable>>,
    pub stack: [u8; 8192],     // Kernel stack
}

impl Process {
    const fn empty() -> Self {
        Self {
            pid: 0,
            state: State::Unused,
            sp: VAddr::new(0),
            page_table: None,
            stack: [0; 8192],
        }
    }
}
```

Lastly, map the kernel pages in the `create_process` function. The kernel pages span from `__kernel_base` to `__free_ram_end`. This approach ensures that the kernel can always access both statically allocated areas (like `.text`), and dynamically allocated areas managed by `alloc`:

```rust [kernel/src/process.rs]
...
unsafe extern "C" {
    static __kernel_base: u8;
    static __free_ram_end: u8;
}
...
pub fn create_process(pc: usize) -> usize {
    ...
    // Map kernel pages.
    let mut page_table = Box::new(PageTable::new());
    let kernel_base = &raw const __kernel_base as usize;
    let free_ram_end = &raw const __free_ram_end as usize;

    for paddr in (kernel_base..free_ram_end).step_by(PAGE_SIZE) {
        map_page(page_table.as_mut(), VAddr::new(paddr), PAddr::new(paddr), PAGE_R | PAGE_W | PAGE_X);
    }

    // Initialise fields.
    process.pid = i + 1;
    process.state = State::Runnable;
    process.sp = VAddr::new(&raw const process.stack[callee_saved_regs_start] as usize);
    process.page_table = Some(page_table);

    process.pid
}
```

## Switching page tables

Let's switch the process's page table when context switching in `scheduler.rs`:

```rust [kernel/src/scheduler.rs]
...
use crate::allocator::PAGE_SIZE;
use crate::page::{SATP_SV32, PageTable};
...
pub fn yield_now() {
    ...
    let (next_sp_ptr, current_sp_ptr, satp, sscratch) = {
        let next_index = PROCS.try_get_index(next_pid)
            .expect("should find next by pid");
        let current_index = PROCS.try_get_index(current_pid)
            .expect("should find current by pid");
        let mut procs = PROCS.0.lock();
        let [next, current] = procs.get_disjoint_mut([next_index, current_index])
            .expect("indices should be valid and distinct");

        let next_sp_ptr = next.sp.field_raw_ptr();
        let current_sp_ptr = current.sp.field_raw_ptr();

        let page_table = next.page_table.as_ref().expect("page_table should exist");
        // Double deref on page_table for both ref and Box.
        let page_table_addr = &**page_table as *const PageTable as usize;
        let satp = SATP_SV32 | (page_table_addr / PAGE_SIZE);
        //Safety: sscratch points to the end of next.stack, which is a valid stack allocation.
        let sscratch = unsafe { next.stack.as_ptr().add(next.stack.len()) };
        (next_sp_ptr, current_sp_ptr, satp, sscratch)
    };

    unsafe{asm!(
        "sfence.vma",
        "csrw satp, {satp}",
        "sfence.vma",
        "csrw sscratch, {sscratch}",
        satp = in(reg) satp,
        sscratch = in(reg) sscratch,
    )};

    // Context switch
    *CURRENT_PROC.lock() = Some(next_pid);
    unsafe {
        switch_context(current_sp_ptr, next_sp_ptr);
    }
}



```
We can switch page tables by specifying the first-level page table in `satp`. Note that we divide by `PAGE_SIZE` because it's the physical page number.

`sfence.vma` instructions added before and after setting the page table serve two purposes:

1. To ensure that changes to the page table are properly completed (similar to a memory fence).
2. To clear the cache of page table entries (TLB).

> [!TIP]
>
> When the kernel starts, paging is disabled by default (the `satp` register is not set). Virtual addresses behave as if they match physical addresses.

## Testing paging

let's try it and see how it works!

```
$ cargo run
Hello World! 🦀
starting process A
🐈starting process B
🐕🐈🐕🐈🐕🐈🐕🐈🐕🐈🐕🐈🐕🐈🐕🐈🐕🐈QEMU: Terminated
```

The output is exactly the same as in the previous chapter (context switching). There's no visible change even after enabling paging. To check if we've set up the page tables correctly, let's inspect it with QEMU monitor!

## Examining page table contents

Let's look at how the virtual addresses around `0x80200000` are mapped. If set up correctly, they should be mapped so that `(virtual address) == (physical address)`.

```
QEMU 8.0.2 monitor - type 'help' for more information
(qemu) stop
(qemu) info registers
 ...
 satp     80080245
 ...
```

You can see that `satp` is `0x80080245`. According to the specification (RISC-V Sv32 mode), interpreting this value gives us the first-level page table's starting physical address: 

```
$ python -c "print(hex((0x80080245 & 0x3fffff) * 4096))"
0x80245000
```

Next, let's inspect the contents of the first-level page table. We want to know the second-level page table corresponding to the virtual address `0x80200000`. QEMU provides commands to display memory contents (memory dump). `xp` command dumps memory at the specified physical address. Dump the 512th entry because `VPN[1] = 0x80200000 >> 22 = 512`. Since each entry is 4 bytes, we multiply by 4:

```
(qemu) xp /x 0x80245000+512*4
0000000080245800: 0x20091801
```

The first column shows the physical address, and the subsequent columns show the memory values. We can see that some non-zero values are set. The `/x` option specifies hexadecimal display. Adding a number before `x` (e.g., `/1024x`) specifies the number of entries to display.

> [!TIP]
>
> Using the `x` command instead of `xp` allows you to view the memory dump for a specified **virtual** address. This is useful when examining user space (application) memory, where virtual addresses do not match physical addresses, unlike in our kernel space.

According to the specification, the second-level page table is located at `(0x20095000 >> 10) * 4096 = 0x80246000`. We again dump its 512th entry because `VPN[0] = (0x80200000 >> 12) & 0x3ff = 512`:

```
(qemu) xp /x 0x80246000+512*4
0000000080246800: 0x2008004f
```

The value `0x2008004f` corresponds to the physical page number `0x2008004f >> 10 = 0x80200` (according to the specification, we ignore the lowest 10 bits, which contain permission flags).
This means that the virtual address `0x80200000` is mapped to the physical address `0x80200000`, as we wanted!

Let's also dump the entire first-level table (1024 entries):

```
(qemu) xp /1024x 0x80245000
0000000080245000: 0x00000000 0x00000000 0x00000000 0x00000000
0000000080245010: 0x00000000 0x00000000 0x00000000 0x00000000
0000000080245020: 0x00000000 0x00000000 0x00000000 0x00000000
0000000080245030: 0x00000000 0x00000000 0x00000000 0x00000000
...
00000000802457f0: 0x00000000 0x00000000 0x00000000 0x00000000
0000000080245800: 0x20091801 0x20091c01 0x20092001 0x20092401
0000000080245810: 0x20092801 0x20092c01 0x20093001 0x20093401
0000000080245820: 0x20093801 0x20093c01 0x20094001 0x20094401
0000000080245830: 0x20094801 0x20094c01 0x20095001 0x20095401
0000000080245840: 0x20095801 0x00000000 0x00000000 0x00000000
0000000080245850: 0x00000000 0x00000000 0x00000000 0x00000000
...
```

The initial entries are filled with zeros, but values start appearing from the 512th entry (`254800`). This is because `__kernel_base` is `0x80200000`, and `VPN[1]` is `0x200`.

We've manually read memory dumps, but QEMU actually provides a command that displays the current page table mappings in human-readable format. If you want to do a final check on whether the mapping is correct, you can use the `info mem` command:

```
(qemu) info mem
vaddr    paddr            size     attr
-------- ---------------- -------- -------
80200000 0000000080200000 00002000 rwx--a-
80202000 0000000080202000 00001000 rwx--ad
80203000 0000000080203000 00001000 rwx----
80204000 0000000080204000 00001000 rwx--ad
80205000 0000000080205000 00001000 rwx----
80206000 0000000080206000 00001000 rwx--ad
80207000 0000000080207000 00001000 rwx----
80208000 0000000080208000 00001000 rwx--a-
80209000 0000000080209000 00001000 rwx----
8020a000 000000008020a000 00001000 rwx--a-
8020b000 000000008020b000 00001000 rwx----
8020c000 000000008020c000 00001000 rwx--a-
8020d000 000000008020d000 00001000 rwx----
8020e000 000000008020e000 00001000 rwx--a-
8020f000 000000008020f000 00001000 rwx----
80210000 0000000080210000 00001000 rwx--a-
80211000 0000000080211000 00001000 rwx----
80212000 0000000080212000 00001000 rwx--ad
80213000 0000000080213000 001ed000 rwx----
80400000 0000000080400000 00400000 rwx----
80800000 0000000080800000 00400000 rwx----
80c00000 0000000080c00000 00400000 rwx----
81000000 0000000081000000 00400000 rwx----
81400000 0000000081400000 00400000 rwx----
81800000 0000000081800000 00400000 rwx----
81c00000 0000000081c00000 00400000 rwx----
82000000 0000000082000000 00400000 rwx----
82400000 0000000082400000 00400000 rwx----
82800000 0000000082800000 00400000 rwx----
82c00000 0000000082c00000 00400000 rwx----
83000000 0000000083000000 00400000 rwx----
83400000 0000000083400000 00400000 rwx----
83800000 0000000083800000 00400000 rwx----
83c00000 0000000083c00000 00400000 rwx----
84000000 0000000084000000 00233000 rwx----
```

The columns represent, in order: virtual address, physical address, size (in hexadecimal bytes), and attributes.

Attributes are represented by a combination of `r` (readable), `w` (writable), `x` (executable), `a` (accessed), and `d` (written), where `a` and `d` indicate that the CPU has "accessed the page" and "written to the page" respectively. They are auxiliary information for the OS to keep track of which pages are actually being used/modified.

> [!TIP]
>
> For beginners, debugging page table can be quite challenging. If things aren't working as expected, refer to the "Appendix: Debugging paging" section.

## Appendix: Debugging paging

Setting up page tables can be tricky, and mistakes can be hard to notice. In this appendix, we'll look at some common paging errors and how to debug them.

### Forgetting to set the paging mode

Let's say we forget to set the mode in the `satp` register:

```rust [kernel/src/process.rs]
        ...
        let satp =  (page_table_addr / PAGE_SIZE); // Missing SATP_SV32!
        ...
```

However, when you run the OS, you'll see that it works as usual. This is because paging remains disabled and memory addresses are treated as physical addresses as before.

To debug this case, try `info mem` command in the QEMU monitor. You'll see something like this:

```
(qemu) info mem
No translation or protection
```

### Specifying physical address instead of physical page number

Let's say we mistakenly specify the page table using a physical *address* instead of a physical *page number*:

```rust [kernel/src/process.rs]
        ...
        let satp = SATP_SV32 | (page_table_addr); // Forgot to shift!
        ...
```
In this case, `info mem` will print no mappings:

```
$ ./run.sh

QEMU 8.0.2 monitor - type 'help' for more information
(qemu) stop
(qemu) info mem
vaddr    paddr            size     attr
-------- ---------------- -------- -------
```

To debug this, dump registers to see what the CPU is doing:

```
(qemu) info registers

CPU#0
 V      =   0
 pc       80200e60
 ...
 scause   0000000c
 ...
```

According to `llvm-objdump`, `80200e60` is the starting address of the exception handler. The exception reason in `scause` corresponds to "Instruction page fault". 

Let's take a closer look at what's specifically happening by examining the QEMU logs:

```bash [run.sh] {2}
$QEMU -machine virt -bios default -nographic -serial mon:stdio --no-reboot \
    -d unimp,guest_errors,int,cpu_reset -D qemu.log \  # new!
    -kernel kernel.elf
```

```
...
Invalid read at addr 0x233000800, size 4, region '(null)', reason: rejected
riscv_cpu_do_interrupt: hart:0, async:0, cause:0000000c, epc:0x80200a98, tval:0x80200a98, desc=exec_page_fault
Invalid read at addr 0x233000800, size 4, region '(null)', reason: rejected
riscv_cpu_do_interrupt: hart:0, async:0, cause:0000000c, epc:0x80200e60, tval:0x80200e60, desc=exec_page_fault
Invalid read at addr 0x233000800, size 4, region '(null)', reason: rejected
riscv_cpu_do_interrupt: hart:0, async:0, cause:0000000c, epc:0x80200e60, tval:0x80200e60, desc=exec_page_fault
Invalid read at addr 0x233000800, size 4, region '(null)', reason: rejected
...
```

Here are what you can infer from the logs:

- `epc`, which indicates the location of the page fault exception, is `0x80200a98`. `llvm-objdump` shows that it points to the instruction immediately after setting the `satp` register. This means that a page fault occurs right after enabling paging.

- All subsequent page faults show the same value. The exceptions occurred at `0x80200e60`, points to the starting address of the exception handler. Because this log continues indefinitely, the exceptions (page fault) occurs when trying to execute the exception handler.

- Looking at the `info registers` in QEMU monitor, `satp` is `0x80233000`. Calculating the physical address according to the specification: `(0x80233000 & 0x3fffff) * 4096 = 0x233000000`, which does not fit within a 32-bit address space. This indicates that an abnormal value has been set.

To summarize, you can investigate what's wrong by checking QEMU logs, register dumps, and memory dumps. However, the most important thing is to _"read the specification carefully."_ It's very common to overlook or misinterpret it.

Here is an example with debug printing. Each process has two pages assigned for table1 and table0 respectively, and then the vpn0 offset in table0 are used to populate the page table entries:

```
create_process: calling map_page ...

map_page: starting mapping ...
map_page: table1 is at address 80234000
map_page: vpn1 is 200
map_page: table0 created at address 0x80235000
map_page: table1[vpn1] is Pte(2008d401)
map_page: table0 recovered from table1[vpn1] PTE starts at address 80235000
map_page: vpn0 is 200
map_page: table0[vpn0] set to Pte(2008000f)

map_page: starting mapping ...
map_page: table1 is at address 80234000
map_page: vpn1 is 200
map_page: table1[vpn1] is Pte(2008d401)
map_page: table0 recovered from table1[vpn1] PTE starts at address 80235000
map_page: vpn0 is 201
map_page: table0[vpn0] set to Pte(2008040f)

map_page: starting mapping ...
map_page: table1 is at address 80234000
map_page: vpn1 is 200
map_page: table1[vpn1] is Pte(2008d401)
map_page: table0 recovered from table1[vpn1] PTE starts at address 80235000
map_page: vpn0 is 202
map_page: table0[vpn0] set to Pte(2008080f)

...

map_page: starting mapping ...
map_page: table1 is at address 80258000
map_page: vpn1 is 210
map_page: table1[vpn1] is Pte(2009a401)
map_page: table0 recovered from table1[vpn1] PTE starts at address 80269000
map_page: vpn0 is 232
map_page: table0[vpn0] set to Pte(2108c80f)

map_page: starting mapping ...
map_page: table1 is at address 80258000
map_page: vpn1 is 210
map_page: table1[vpn1] is Pte(2009a401)
map_page: table0 recovered from table1[vpn1] PTE starts at address 80269000
map_page: vpn0 is 233
map_page: table0[vpn0] set to Pte(2108cc0f)

yield_now: satp is being set to 80080234
yield_now: sscratch is being set to 0x8020517c
```
