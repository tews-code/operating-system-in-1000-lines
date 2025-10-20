//! Round-robin scheduler

use core::arch::asm;

use crate::allocator::PAGE_SIZE;
use crate::page::{SATP_SV32, PageTable};
use crate::process::{create_process, PROCS, PROCS_MAX, State, switch_context};
use crate::spinlock::SpinLock;

static IDLE_PROC: SpinLock<Option<usize>> = SpinLock::new(None);    // Idle process
static CURRENT_PROC: SpinLock<Option<usize>> = SpinLock::new(None); // Currently running process
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
        let satp = SATP_SV32 | (page_table_addr);
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
