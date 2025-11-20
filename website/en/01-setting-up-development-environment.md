---
title: Getting Started
---

# Getting Started

This book assumes you're using a UNIX or UNIX like OS such as macOS or Ubuntu. If you're on Windows, install Windows Subsystem for Linux (WSL2) and follow the Ubuntu instructions.

## Install development tools

### macOS 

Install [Homebrew](https://brew.sh) and run this command to get all tools you need:

```
brew install rustup llvm lld qemu
```

Also, you need to add LLVM binutils to your `PATH`:

```
$ export PATH="$PATH:$(brew --prefix)/opt/llvm/bin"
$ which llvm-objcopy
/opt/homebrew/opt/llvm/bin/llvm-objcopy
```

### Ubuntu

Install packages with `apt`:

```
sudo apt update && sudo apt install -y rustup llvm lld qemu-system-riscv32 curl
```

### Fedora

Install packages with `dnf`:

```
sudo dnf update && sudo dnf install -y rustup llvm lld qemu-system-riscv32 curl
```

### OpenSBI

Also, download OpenSBI (think of it as BIOS/UEFI for PCs):

```
curl -LO https://github.com/qemu/qemu/raw/v8.0.4/pc-bios/opensbi-riscv32-generic-fw_dynamic.bin
```

### Rust

Rust is configured using the `rustup` tool. We will be cross compling to RISC-V 32-bit binaries, so we need the associated toolchain:

You will need to initialise the Rust toolchain using `rustup-init`.

```
$ rustup-init
```

Following the `rustup-init` instructions, restart your terminal to take advantage of the new PATH additions.

Now add the 32-bit RISC-V target for our new operating system.

```
$ rustup target add riscv32imac-unknown-none-elf
```

If you are using an editor that can use Language Server Protocal such as VSCode, you may want to install `rust-analyzer`.

```
$ rustup component add rust-analyzer
```

> [!WARNING]
>
> When you run QEMU, make sure `opensbi-riscv32-generic-fw_dynamic.bin` is in your current directory. If it's not, you'll see this error:
>
> ```
> qemu-system-riscv32: Unable to load the RISC-V firmware "opensbi-riscv32-generic-fw_dynamic.bin"
> ```

### Other OS users

If you are using other OSes, get the following tools:

- `bash`: The command-line shell. Usually it's pre-installed.
- `tar`: Usually it's pre-installed. Prefer GNU version, not BSD.
- `rustc`: Rust compiler. Make sure it supports 32-bit RISC-V CPU (see below).
- `cargo`: Rust package manager.
- `lld`: LLVM linker, which bundles complied object files into an executable.
- `llvm-objcopy`: Object file editor. It comes with the LLVM package (typically `llvm` package).
- `llvm-objdump`: A disassembler. Same as `llvm-objcopy`.
- `llvm-readelf`: An ELF file reader. Same as `llvm-objcopy`.
- `qemu-system-riscv32`: 32-bit RISC-V CPU emulator. It's part of the QEMU package (typically `qemu` package).

> [!TIP]
>
> To check if your Rust install supports 32-bit RISC-V CPU, run this command:
>
> ```
> $ rustc --print target-list | grep riscv32
>     ...
>     riscv32imac-unknown-none-elf
>     ...
> ```
>
> You should see `riscv32imac` that we will be using, as well as many other riscv32 targets. 

## Setting up a Git repository (optional)

If you're using a Git repository, use the following `.gitignore` file:

```gitignore [.gitignore]
target/
kernel.elf
kernel/kernel.map
user/user.map
*.map
*.tar
*.o
*.elf
*.bin
*.log
*.pcap
*.epub
```

You're all set! Let's start building your first operating system!
