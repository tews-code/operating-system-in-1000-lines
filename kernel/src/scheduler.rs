//! Round-robin scheduler

use crate::process::{create_process, PROCS, PROCS_MAX, State, switch_context};
use crate::spinlock::SpinLock;


static IDLE_PROC: SpinLock<Option<usize>> = SpinLock::new(None);    // Idle process
static CURRENT_PROC: SpinLock<Option<usize>> = SpinLock::new(None); // Currently running process
const IDLE_PID: usize = 0; // idle

pub fn yield_now() {
    // Initialse IDLE_PROC if not yet initialised
    IDLE_PROC.lock().get_or_insert_with(|| {
            let idle_pid = create_process(0);
            if let Some(p) = PROCS.0.lock().iter_mut()
                .find(|p| p.pid == idle_pid) {
                    p.pid = IDLE_PID;
                }
            *CURRENT_PROC.lock() = Some(IDLE_PID);
            IDLE_PID
        }
    );

    let idle_pid = IDLE_PROC.lock()
        .expect("IDLE_PROC initialised before use");
    let current_pid = CURRENT_PROC.lock()
        .expect("CURRENT_PROC initialised before use");

    // Search for a runnable process
    let next_pid = {
        let current_index = PROCS.index(current_pid)
            .expect("current process PID should have an index");
        PROCS.0.lock().iter()
            .cycle()
            .skip(current_index + 1)
            .take(PROCS_MAX)
            .find(|p| p.state == State::Runnable && p.pid != idle_pid)
            .map(|p| p.pid)
            .unwrap_or_else(|| idle_pid)
    };

    // If there's no runnable process other than the current one, return and continue processing
    if next_pid == current_pid {
        return;
    }

    // Context switch
    *CURRENT_PROC.lock() = Some(next_pid);
    let (current_sp_ptr, next_sp_ptr) = PROCS
        .get_disjoint_sp_ptrs(current_pid, next_pid)
        .expect("failed to get stack pointers for context switch");
    unsafe {
        switch_context(current_sp_ptr, next_sp_ptr);
    }
}
