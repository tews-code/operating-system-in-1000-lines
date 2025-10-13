# What we will implement

Before starting to build an OS, let's quickly get an overview of the features we will implement.

## Features in 1K LoC OS

In this book, we will implement the following major features:

- **Multitasking**: Switch between processes to allow multiple applications to share the CPU.
- **Exception handler**: Handle events requiring OS intervention, such as illegal instructions.
- **Paging**: Provide an isolated memory address space for each application.
- **System calls**: Allow applications to call kernel features.
- **Device drivers**: Abstract hardware functionalities, such as disk read/write.
- **File system**: Manage files on disk.
- **Command-line shell**: User interface for humans.

## Features not implemented

The following major features are not implemented in this book:

- **Interrupt handling**: Instead, we will use a polling method (periodically check for new data on devices), also known as busy waiting.
- **Timer processing**: Preemptive multitasking is not implemented. We'll use cooperative multitasking, where each process voluntarily yields the CPU.
- **Inter-process communication**: Features such as pipe, UNIX domain socket, and shared memory are not implemented.
- **Multi-processor support**: Only single processor is supported.

## Cargo and source code structure

Rust comes with a package manager Cargo, which allows us to structure and control our project. We will use Cargo commands to create, compile and run our code. Cargo should already be installed by `rustup`, and you can check using: 

```bash
$ cargo --version
```

We'll build from scratch incrementally, using the Rust package manager Cargo. and the final file structure will look like this:

```
os1k                     - Root folder and operating system name
├── disk/                - File system contents
├── Cargo.toml           - Cargo workspace configuration
├── common               - Package for common library
|   ├── build.rs         - Common: Cargo build script for common package
│   ├── Cargo.toml       - Common: Cargo package configuration
│   └── src              - Common: source files
│       ├── lib.rs       - Common main library
│       └── print.rs     - Common print module
├── kernel               - Kernel package
│   ├── build.rs         - Kernel: Build script for kernel
│   ├── Cargo.toml       - Kernel: Cargo package configuration
│   ├── kernel.ld        - Kernel: linker script (memory layout definition)
│   └── src              - Kernel: source files
│       ├── address.rs   - Address helper functions
│       ├── allocator.rs - Heap memory allocator
│       ├── entry.rs     - Exception and syscall handling
│       ├── lib.rs       - Kernel: main library
│       ├── main.rs      - Kernel: main binary
│       ├── page.rs      - Memory page handling
│       ├── panic.rs     - Panic handling
│       ├── policy.rs    - Scheduler policy
│       ├── process.rs   - Process creation
│       ├── sbi.rs       - SBI interface
│       ├── scheduler.rs - Scheduler
│       ├── tar.rs       - TAR file support
│       └── virtio.rs    - VirtIO driver
├── user                 - User (application) package
|   ├── build.rs         - User: Build script for user applications
|   ├── Cargo.toml       - User: Cargo package configuration
|   ├── src              - User: source files
|   │   ├── bin          - User: Binary source files
|   │   │   └── shell.rs - Command line shell
|   │   ├── lib.rs       - User: main library
|   │   └── syscall.rs   - User library: functions for system calls
|   └── user.ld          - User: linker script (memory layout definition)
├── ./cargo              - Workspace config folder
|   └── config.toml      - Workspace config file
├── build.sh             - Build script
└── run.sh               - Runner script
```

> [!TIP]
>
> In this book, "user land" is sometimes abbreviated as "user". Consider it as "applications", and do not confuse it with "user account"!

## Create the workspace 

In Rust, the folder name of your project matters! Choose a name for your operating system, and create the folder. In this tutorial, we will call our operating system `os1k`, but you can choose anything you like.

```bash
$ mkdir os1k && cd os1k
```
We want to set the target of the entire workspace to be RISC-V 32-bit, so we need to create a configuration TOML file.

```bash
$ mkdir .cargo
$ touch .cargo/config.toml
```

In the configuration file we simply add the build target triple and compiler flags, as well as asking Cargo to execute `run.sh` when we use `cargo run`:

```toml [.cargo/config.toml]
[build]
target="riscv32imac-unknown-none-elf"
rustflags = ["-g", "-O"]

[target.riscv32imac-unknown-none-elf]
runner = "./run.sh"
```
The specified Cargo compiler options are as follows:

| Option | Description |
| ------ | ----------- |
| [build] | Options relating to build |
| `-O` | Enable optimizations to generate efficient machine code. Equivalent to `-C optlevel=3`. |
| `-g` | Generate the maximum amount of debug information. Equivalent to `-C debuginfo=2`. |
| `target = "riscv32imac-unknown-elf"` | Compile for 32-bit RISC-V with IMAC extensions. |
| `[target.riscv32imac-unknown-none-elf]` | Options specifically for this target. |
| `runner = "./run.sh"` | Use our run script to run the compiled code in QEMU. |

Then create a Cargo.toml file to describe our workspace

```bash
$ touch Cargo.toml
```
with just one package in our workspace for now.

```toml [Cargo.toml]
[workspace]
members = ["kernel"]
resolver = "3"
```

With that done, let's get cracking!
