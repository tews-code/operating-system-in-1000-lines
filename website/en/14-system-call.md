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

Next, update the trap handler to handle `ecall` instruction:

```rust [kernel/src/entry.rs] {2, 10-16}
...
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

Do you remember the implementation of the `println!` macro in `common`? It calls the `put_char` function to display characters. Since we have just implemented `put_char` in the userland library, we can use it as is:

```rust [user/src/bin/shell.rs]
```

```c [shell.c] {2}
void main(void) {
    printf("Hello World from shell!\n");
}
```

You'll see the charming message on the screen:

```
$ ./run.sh
Hello World from shell!
```

Congratulations! You've successfully implemented the system call! But we're not done yet. Let's implement more system calls!

## Receive characters from keyboard (`getchar` system call)

Our next goal is to implement a shell. To do that, we need to be able to receive characters from the keyboard.

SBI provides an interface to read "input to the debug console". If there is no input, it returns `-1`:

```c [kernel.c]
long getchar(void) {
    struct sbiret ret = sbi_call(0, 0, 0, 0, 0, 0, 0, 2);
    return ret.error;
}
```

The `getchar` system call is implemented as follows:

```c [common.h]
#define SYS_GETCHAR 2
```

```c [user.c]
int getchar(void) {
    return syscall(SYS_GETCHAR, 0, 0, 0);
}
```

```c [user.h]
int getchar(void);
```

```c [kernel.c] {3-13}
void handle_syscall(struct trap_frame *f) {
    switch (f->a3) {
        case SYS_GETCHAR:
            while (1) {
                long ch = getchar();
                if (ch >= 0) {
                    f->a0 = ch;
                    break;
                }

                yield();
            }
            break;
        /* omitted */
    }
}
```

The implementation of the `getchar` system call repeatedly calls the SBI until a character is input. However, simply repeating this prevents other processes from running, so we call the `yield` system call to yield the CPU to other processes.

> [!NOTE]
>
> Strictly speaking, SBI does not read characters from keyboard, but from the serial port. It works because the keyboard (or QEMU's standard input) is connected to the serial port.

## Write a shell

Let's write a shell with a simple command `hello`, which displays `Hello world from shell!`:

```c [shell.c]
void main(void) {
    while (1) {
prompt:
        printf("> ");
        char cmdline[128];
        for (int i = 0;; i++) {
            char ch = getchar();
            putchar(ch);
            if (i == sizeof(cmdline) - 1) {
                printf("command line too long\n");
                goto prompt;
            } else if (ch == '\r') {
                printf("\n");
                cmdline[i] = '\0';
                break;
            } else {
                cmdline[i] = ch;
            }
        }

        if (strcmp(cmdline, "hello") == 0)
            printf("Hello world from shell!\n");
        else
            printf("unknown command: %s\n", cmdline);
    }
}
```

It reads characters until a newline comes, and checks if the entered string matches the command name.

> [!WARNING]
>
> Note that on the debug console, the newline character is (`'\r'`).

Let's try typing `hello` command:

```
$ ./run.sh

> hello
Hello world from shell!
```

Your OS is starting to look like a real OS! How fast you've come this far!

## Process termination (`exit` system call)

Lastly, let's implement `exit` system call, which terminates the process:

```c [common.h]
#define SYS_EXIT    3
```

```c [user.c] {2-3}
__attribute__((noreturn)) void exit(void) {
    syscall(SYS_EXIT, 0, 0, 0);
    for (;;); // Just in case!
}
```

```c [kernel.h]
#define PROC_EXITED   2
```

```c [kernel.c] {3-7}
void handle_syscall(struct trap_frame *f) {
    switch (f->a3) {
        case SYS_EXIT:
            printf("process %d exited\n", current_proc->pid);
            current_proc->state = PROC_EXITED;
            yield();
            PANIC("unreachable");
        /* omitted */
    }
}
```

The system call changes the process state to `PROC_EXITED`, and calls `yield` to give up the CPU to other processes. The scheduler will only execute processes in `PROC_RUNNABLE` state, so it will never return to this process. However, `PANIC` macro is added to cause a panic in case it does return.

> [!TIP]
>
> For simplicity, we only mark the process as exited (`PROC_EXITED`). If you want to build a practical OS, it is necessary to free resources held by the process, such as page tables and allocated memory regions.

Add the `exit` command to the shell:

```c [shell.c] {3-4}
        if (strcmp(cmdline, "hello") == 0)
            printf("Hello world from shell!\n");
        else if (strcmp(cmdline, "exit") == 0)
            exit();
        else
            printf("unknown command: %s\n", cmdline);
```

You're done! Let's try running it:

```
$ ./run.sh

> exit
process 2 exited
PANIC: kernel.c:333: switched to idle process
```

When the `exit` command is executed, the shell process terminates via system call, and there are no other runnable processes remaining. As a result, the scheduler will select the idle process and cause a panic.
