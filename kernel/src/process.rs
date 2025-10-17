//! Process

use core::arch::naked_asm;
use core::fmt;

use crate::address::VAddr;
use crate::spinlock::SpinLock;

pub const PROCS_MAX: usize = 8;    // Maximum number of processes

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum State {
    Unused,     // Unused process control structure
    Runnable,   // Runnable process
}

#[derive(Copy, Clone)]
pub struct Process {
    pub pid: usize,            // Process ID
    pub state: State,              // Process state: Unused or Runnable
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

pub struct Procs(pub SpinLock<[Process; PROCS_MAX]>);

impl Procs {
    const fn new() -> Self {
        Self(
            SpinLock::new([Process::empty(); PROCS_MAX])
        )
    }

    pub fn index(&self, pid: usize) -> Option<usize> {
        self.0.lock().iter().position(|p| p.pid == pid)
    }

    pub fn get_disjoint_sp_ptrs(&self, pid_a: usize, pid_b: usize) -> Option<(*mut usize, *mut usize)> {
        let mut procs = self.0.lock();

        let index_a = procs.iter().position(|p| p.pid == pid_a)?;
        let index_b = procs.iter().position(|p| p.pid == pid_b)?;

        debug_assert_ne!(index_a, index_b, "processes must be different");
        // Method allows us to get two &mut Process from the one Procs array at the same time
        let [proc_a, proc_b] = procs.get_disjoint_mut([index_a, index_b]).ok()?;

        Some((proc_a.sp.as_ptr_mut(), proc_b.sp.as_ptr_mut()))
    }
}

impl fmt::Display for Procs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let procs = PROCS.0.lock();
        for (i, process) in procs.iter().enumerate() {
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
