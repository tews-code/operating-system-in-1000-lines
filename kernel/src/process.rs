//! Process

use alloc::slice;
use alloc::boxed::Box;

use core::arch::{asm, naked_asm};
use core::fmt;

use crate::address::{align_up, PAddr, VAddr};
use crate::allocator::PAGE_SIZE;
use crate::page::{map_page, PageTable, PAGE_R, PAGE_W, PAGE_X, PAGE_U};
use crate::spinlock::SpinLock;

unsafe extern "C" {
    static __kernel_base: u8;
    static __free_ram_end: u8;
}

pub const PROCS_MAX: usize = 8;         // Maximum number of processes

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum State {
    Unused,     // Unused process control structure
    Runnable,   // Runnable process
    Exited,
}

#[derive(Clone, Debug)]
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
impl fmt::Display for Procs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let procs = PROCS.0.lock();
        for (i, process) in procs.iter().enumerate() {
            write!(f, "Addr: {:x?} ", &raw const *process as usize)?;
            writeln!(f, "PROC[{i}]")?;
            write!(f, "PID: {} ", process.pid)?;
            write!(f, "SP: {:x?} ", process.sp)?;
            writeln!(f, "STATE: {:?} ", process.state)?;
            writeln!(f, "STACK: [ ... {:x?}]", &process.stack[8140..8191])?
        }
        Ok(())
    }
}

pub static PROCS: Procs = Procs::new();  // All process control structures.

// The base virtual address of an application image. This needs to match the
// starting address defined in `user.ld`.
const USER_BASE: usize = 0x1000000;
const SSTATUS_SPIE: usize =  1 << 5;    // Enable user mode

fn user_entry() {
    unsafe{asm!(
        "csrw sepc, {sepc}",
        "csrw sstatus, {sstatus}",
        "sret",
        sepc = in(reg) USER_BASE,
        sstatus = in(reg) SSTATUS_SPIE,
    )}
}

pub fn create_process(image: *const u8, image_size: usize) -> usize {
    let mut procs = PROCS.0.lock();

    // Find an unused process control structure.
    let (i, process) = procs.iter_mut()
        .enumerate()
        .find(|(_, p)| p.state == State::Unused)
        .expect("no free process slots");

    // Stack callee-saved registers. These register values will be restored in
    // the first context switch in switch_context.
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

    // Place the callee-saved registers at the end of the stack
    let callee_saved_regs_start = process.stack.len() - callee_saved_regs.len() * size_of::<usize>();
    let mut offset = callee_saved_regs_start;
    for reg in &callee_saved_regs {
        let bytes = reg.to_ne_bytes(); // native endian
        process.stack[offset..offset + size_of::<usize>()].copy_from_slice(&bytes);
        offset += size_of::<usize>();
    }

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

    // Initialise fields.
    process.pid = i + 1;
    process.state = State::Runnable;
    process.sp = VAddr::new(&raw const process.stack[callee_saved_regs_start] as usize);

    process.pid
}

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
