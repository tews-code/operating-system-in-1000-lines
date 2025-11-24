# System Call

In this chapter, we will implement *"system calls"* that allow applications to invoke kernel functions. Time to Hello World from the userland!

## User library

Invoking system call is quite similar to [the SBI call implementation](/en/05-hello-world#say-hello-to-sbi) we've seen before. Let's add this to the library at `user/src/lib.rs`.

```rust [user/src/lib.rs]
use common::{
    SYS_PUTBYTE,
};

pub fn sys_call(sysno: usize, arg0: isize, arg1: isize, arg2: isize, arg3: isize) -> isize {
    let a0: isize;
    unsafe{asm!(
        "ecall",
        inout("a0") arg0 => a0,
        in("a1") arg1,
        in("a2") arg2,
        in("a3") arg3,
        in("a4") sysno,
    )}
    a0
}

#[unsafe(no_mangle)]
pub fn put_byte(b: u8) -> Result<(), isize> {
    let result = sys_call(SYS_PUTBYTE, b as isize, 0, 0, 0);
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}
```

The `syscall` function sets the system call number in the `a4` register and the system call arguments in the `a0` to `a3` registers, then executes the `ecall` instruction. The `ecall` instruction is a special instruction used to delegate processing to the kernel. When the `ecall` instruction is executed, an exception handler is called, and control is transferred to the kernel. The return value from the kernel is set in the `a0` register.

The first system call we will implement is `put_byte`, which outputs a byte, via system call. It takes a character as the first argument. For the second and subsequent unused arguments are set to 0. 

We add this to our common library, so that both the kernel and user library use the same definition. Add this to `common/src/lib.rs`:

```rust [common/src/lib.rs]
...
pub const SYS_PUTBYTE: usize = 1;
...
```

## Handle `ecall` instruction in the kernel

Next, update the trap handler to handle `ecall` instruction in `kernel/src/entry.rs`:

```rust [kernel/src/entry.rs] {2, 10-16}
...
use common::SYS_PUTBYTE;

use crate::sbi::put_byte;
use crate::{read_csr, write_csr};

const SCAUSE_ECALL: usize = 8;
...
#[unsafe(no_mangle)]
extern "C" fn handle_trap(trap_frame: &mut TrapFrame) {
    let scause = read_csr!("scause");
    let stval = read_csr!("stval");
    let mut user_pc = read_csr!("sepc");

    if scause == SCAUSE_ECALL {
        handle_syscall(trap_frame);
        user_pc += 4;
    } else {
        panic!("unexpected trap scause={:x}, stval={:x}, sepc={:x}", scause,  stval, user_pc);
    }

    write_csr!("sepc", user_pc);
}
```
Whether the `ecall` instruction was called can be determined by checking the value of `scause`. Besides calling the `handle_syscall` function, we also add 4 (the size of `ecall` instruction) to the value of `sepc`. This is because `sepc` points to the program counter that caused the exception, which points to the `ecall` instruction. If we don't change it, the kernel goes back to the same place, and the `ecall` instruction is executed repeatedly.

## System call handler

The following system call handler is called from the trap handler. It receives a structure of "registers at the time of exception" that was saved in the trap handler:

```rust [kernel/src/entry.rs]
fn handle_syscall(f: &mut TrapFrame) {
    let sysno = f.a4;
    match sysno {
        SYS_PUTBYTE => {  // Match what user code sends
            match put_byte(f.a0 as u8) {
                Ok(_) => f.a0 = 0,     // Set return value to 0 (success)
                Err(e) => f.a0 = e as usize,    // Set return value to error code
            }
        },
        _ => {panic!("unexpected syscall sysno={:x}", sysno);},
    }
}
```

It determines the type of system call by checking the value of the `a4` register. Now we only have one system call, `SYS_PUTCHAR`, which simply outputs the byte stored in the `a0` register.

## Test the system call

You've implemented the system call. Let's try it out!

Do you remember the implementation of the `println!` macro in `common`? It calls the `put_char` function to display characters. Since we have just implemented `put_char` in the userland library, we can use it as is in `user/src/bin/shell.rs`:

```rust [user/src/bin/shell.rs]
//! os1k shell

#![no_std]
#![no_main]

use user::println;

#[unsafe(no_mangle)]
fn main() {
    println!("Hello world from the shell!");

    loop {}
}
```

You'll see the charming message on the screen:

```
$ ./os1k.sh
Hello world from the shell!
```

Congratulations! You've successfully implemented the system call! But we're not done yet. Let's implement more system calls!

## Receive characters from keyboard (`get_char` system call)

Our next goal is to implement a shell. To do that, we need to be able to receive characters from the keyboard.

SBI provides an interface to read "input to the debug console". If there is no input, it returns `-1`. Add this to `kernel/src/sbi.rs`:

```rust [kernel/src/sbi.rs] {2-3, 13-19}
...
const EID_CONSOLE_PUTCHAR: c_long = 1;
const EID_CONSOLE_GETCHAR: c_long = 2;
...
pub unsafe fn sbi_call(mut arg0: c_int, eid: c_long) -> Result<isize, isize> {
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") arg0,
            in("a7") eid,
        );
    }

    match eid {
        EID_CONSOLE_PUTCHAR => {
            if arg0 == 0 {
                Ok(0)
            } else {
                Err(arg0 as isize)
            }
        },
        EID_CONSOLE_GETCHAR => {
            if arg0 != -1 {
                Ok(arg0 as isize)
            } else {
                Err(-1)
            }
        },
        _ => {
            panic!("Unknown SBI EID");
        }
    }
}

pub fn get_char() -> Result<isize, isize> {
    unsafe {
        sbi_call(0, EID_CONSOLE_GETCHAR)
    }
}
```

The `get_char` system call is implemented as follows: First, add the command to the common library:

```rust [common/src/lib.rs] {3}
...
pub const SYS_PUTBYTE: usize = 1;
pub const SYS_GETCHAR: usize = 2;

```
And then add the system call to the user library:

```rust [user/src/lib.rs]
...
pub fn get_char() -> Option<usize> {
    let ch = sys_call(SYS_GETCHAR, 0, 0, 0, 0);
    if ch == -1 {
        None
    } else {
        Some(ch as usize)
    }
}

```
And then add the handler in the kernel entry file:

```rust [kernel/src/entry.rs] {2-8, 10, 19-27}]
...
use common::{
    SYS_PUTBYTE,
    SYS_GETCHAR,
};

use crate::sbi::{put_byte, get_char};
use crate::scheduler::yield_now;
...
fn handle_syscall(f: &mut TrapFrame) {
    let sysno = f.a4;
    match sysno {
        SYS_PUTBYTE => {  // Match what user code sends
            match put_byte(f.a0 as u8) {
                Ok(_) => f.a0 = 0,     // Set return value to 0 (success)
                Err(e) => f.a0 = e as usize,    // Set return value to error code
            }
        },
        SYS_GETCHAR => {
            loop {
                if let Ok(ch) = get_char() {
                    f.a0 = ch as usize;
                    break;
                }
                yield_now();
            }
        },

```

The implementation of the `get_char` system call repeatedly calls the SBI until a character is input. However, simply repeating this prevents other processes from running, so we call the `yield` system call to yield the CPU to other processes.

> [!NOTE]
>
> Strictly speaking, SBI does not read characters from keyboard, but from the serial port. It works because the keyboard (or QEMU's standard input) is connected to the serial port.

## Write a shell

Let's write a shell with a simple command `hello`, which displays `Hello world from shell!`:

```rust [user/src/bin/shell.rs]
...
fn main() {
    loop {
        print!("> ");
        let mut cmdline = [b'\n'; 128];
        let mut pos = 0;
        loop {
            let Some(ch) = get_char() else {
                break;
            };
            let byte = ch as u8;
            let _ = unsafe{put_byte(byte)};
            match byte {
                b'\r' => { // On the debug console the newline is \r
                    println!();
                    break;
                },
                _ => {
                    cmdline[pos] = byte;
                    pos += 1;
                }
            }
        }

        let cmdline_str = str::from_utf8(&cmdline)
            .expect("command line text valid UTF8")
            .trim();

        match cmdline_str {
            "hello" => {
                println!("Hello world from the shell! 🐚");
            },
            _ => {
                println!("unknown command: {}", cmdline_str);
            },
        }
    }
}

```

It reads characters until a newline comes, and checks if the entered string matches the command name.

> [!WARNING]
>
> Note that on the debug console, the newline character is (`'\r'`).

Let's try typing `hello` command:

```
$ ./os1k.sh

> hello
Hello world from the shell! 🐚
```

Your OS is starting to look like a real OS! How fast you've come this far!

## Process termination (`exit` system call)

Lastly, let's implement `exit` system call, which terminates the process:

In `common` we add the new system call:

```rust [common/src/lib.rs]
pub const SYS_EXIT: usize = 3;
```
And in `user` we add the system call function:

```rust [user/src/lib.rs]

pub fn exit() -> ! {
    let _ = sys_call(SYS_EXIT, 0, 0, 0, 0);
    unreachable!("just in case!");
}
```
On the kernel side, we first create a new process state of `Exited` in `process`:

```rust [kernel/src/process.rs] {5}
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum State {
    Unused,     // Unused process control structure
    Runnable,   // Runnable process
    Exited,
}
```

Then we add the handler in `entry.rs`:

```rust [kernel/src/entry.rs] {4, 7, 9, 13-22}
use common::{
    SYS_PUTBYTE,
    SYS_GETCHAR,
    SYS_EXIT
};

use crate::process::{PROCS, State};
use crate::sbi::{put_byte, get_char};
use crate::scheduler::{yield_now, CURRENT_PROC};
...
fn handle_syscall(f: &mut TrapFrame) {
    ...
         SYS_EXIT => {
            let current = CURRENT_PROC.lock()
                .expect("current process should be running");
            crate::println!("process {} exited", current);
            if let Some(p) = PROCS.0.lock().iter_mut()
                .find(|p| p.pid == current) {
                    p.state = State::Exited
                }
            yield_now();
            unreachable!("unreachable after SYS_EXIT");
        }
        /* Omitted */
```


The system call changes the process state to `State::Exited`, and calls `yield_now` to give up the CPU to other processes. The scheduler will only execute processes in `Runnable` state, so it will never return to this process. However, `unreachable` macro is added to cause a panic in case it does return.

> [!TIP]
>
> For simplicity, we only mark the process as exited (`State::Exited`). If you want to build a practical OS, it is necessary to free resources held by the process, such as page tables and allocated memory regions.

Add the `exit` command to the shell:

```rust [user/src/bin/shell.rs] {2, 11-13}
 use user::{
    exit,
    print,
...

fn handle_syscall(f: &mut TrapFrame) {
    ...
        match cmdline_str {
            "hello" => {
                println!("Hello world from the shell! 🐚");
            },
            "exit" => {
                exit();
            }
            _ => {
                println!("unknown command: {}", cmdline_str);
            },
```

You're done! Let's try running it:

```
$ ./os1k.sh

> exit
process 1 exited
⚠️ Panic: panicked at kernel/src/main.rs:111:5:
switched to idle process

```

When the `exit` command is executed, the shell process terminates via system call, and there are no other runnable processes remaining. As a result, the scheduler will select the idle process and cause a panic.
