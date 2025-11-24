# File System

You've done a great job so far! You've implemented a process, a shell, memory management, and a disk driver. Let's finish up by implementing a file system.

## Tar as file system

In this book, we'll take an interesting approach to implement a file system: using a tar file as our file system.

Tar is an archive format that can contain multiple files. It contains file contents, filenames, creation dates, and other information necessary for a file system. Compared to common file system formats like FAT or ext2, tar has a much simpler data structure. Additionally, you can manipulate the file system image using the  tar command which you are already familiar with. Isn't it an ideal file format for educational purposes?

> [!TIP]
>
> Nowadays, tar is used as a ZIP alternative, but originally it was born as sort of file system for magnetic tape. We can use it as a file system as we do in this chapter, however, you'll notice that it is not suitable for random access. [The design of FAT file system](https://en.wikipedia.org/wiki/Design_of_the_FAT_file_system) would be fun to read.

## Create a disk image (tar file)

Let's start by preparing the contents of our file system. Create a directory called `disk` and add some files to it. Name one of them `hello.txt`:

```
$ mkdir disk
$ vim disk/hello.txt
$ vim disk/meow.txt
```

Add a command to the build script to create a tar file and pass it as a disk image to QEMU:

```bash [run.sh] {1,5}
(cd disk && tar cf ../disk.tar --format=ustar *.txt)                          # new

$QEMU -machine virt -bios default -nographic -serial mon:stdio --no-reboot \
    -d unimp,guest_errors,int,cpu_reset -D qemu.log \
    -drive id=drive0,file=disk.tar,format=raw,if=none \                         # modified
    -device virtio-blk-device,drive=drive0,bus=virtio-mmio-bus.0 \
    -kernel kernel.elf
```

The `tar` command options used here are:

- `cf`: Create tar file.
- `--format=ustar`: Create in ustar format.

> [!TIP]
>
> The parentheses `(...)` create a subshell so that `cd` doesn't affect in other parts of the script.

## Tar file structure

A tar file has the following structure:

```
+----------------+
|   tar header   |
+----------------+
|   file data    |
+----------------+
|   tar header   |
+----------------+
|   file data    |
+----------------+
|      ...       |
```

In summary, a tar file is essentially a series of "tar header" and "file data" pair, one pair for each file. There are several types of tar formats, but we will use the **ustar format** ([Wikipedia](<https://en.wikipedia.org/wiki/Tar_(computing)#UStar_format>)).

We use this file structure as the data structure for our file system. Comparing this to a real file system would be very interesting and educational.

## Reading the file system

First, define the data structures related to tar file system in `tar.rs`:

```rust [kernel/src/tar.rs]
pub const FILES_MAX: usize = 2;
const DISK_MAX_SIZE: usize = align_up(size_of::<File>() * FILES_MAX, SECTOR_SIZE);

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct TarHeader {
    name: [u8; 100],
    mode: [u8; 8],
    uid: [u8; 8],
    gid: [u8; 8],
    size: [u8; 12],
    mtime: [u8; 12],
    checksum: [u8; 8],
    typeflag: u8,
    linkname: [u8; 100],
    magic: [u8; 6],
    version: [u8; 2],
    uname: [u8; 32],
    gname: [u8; 32],
    devmajor: [u8; 8],
    devminor: [u8; 8],
    prefix: [u8; 155],
    _padding: [u8; 12],
    // data follows as a byte array size `size`
}

impl TarHeader {
    fn size(&self) -> usize {
        size_of::<Self>()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct File {
    in_use: bool,
    pub name: [u8; 100],
    pub data: [u8; 1024],
    pub size: usize,
}

impl File {
    const fn zeroed() -> Self {
        // SAFETY: VirtioVirtq contains only structs/arrays of integers and pointers.
        // All-zero bytes is a valid representation: integers become 0, pointer becomes null.
        unsafe { core::mem::MaybeUninit::zeroed().assume_init() }
    }
}
```
In our file system implementation, all files are read from the disk into memory at boot. `FILES_MAX` defines the maximum number of files that can be loaded, and `DISK_MAX_SIZE` specifies the maximum size of the disk image.

Next, let's read the whole disk into memory in `tar`:

```rust [kernel/src/tar.rs]
#[derive(Debug)]
pub struct Files(pub SpinLock<[File; FILES_MAX]>);

//Safety: Single threaded OS
unsafe impl Sync for Files {}

pub static FILES: Files = Files(SpinLock::new([File::zeroed(); FILES_MAX]));

#[derive(Debug)]
pub struct Disk(SpinLock<[u8; DISK_MAX_SIZE]>);

//Safety: Single threaded OS
unsafe impl Sync for Disk {}

impl Disk {
    const fn empty() -> Self {
        Self(SpinLock::new([0u8; DISK_MAX_SIZE]))
    }
}

pub static DISK: Disk = Disk::empty();

fn oct2int(oct: &[u8]) -> Result<usize, ()> {
    oct.iter()
    .take_while(|&&b | b != 0)  // Nul terminated octal slice so stop here
    .try_fold(0, | dec, &b | {
        match b {
            b'0'..=b'7' => Ok(dec * 8 + (b - b'0') as usize),
              _ => Err(())
        }
    })
}


// Turn the file size into a nul terminated octal string.
fn int2oct(dec: usize, oct: &mut [u8]) {
    let mut num = dec;
    oct.fill(b' ');  // Fill with spaces
    if let Some(last_byte) = oct.last_mut() {
        *last_byte = b'\0'; // Set last byte to nul terminator
    }
    oct.iter_mut()
    .rev()
    .skip(1) // Skip the last byte to leave as nul terminator
    .for_each(|byte| {
        *byte = (num % 8) as u8 + b'0';
    num /= 8;
    });
}

pub fn fs_init() {
    // Load into DISK by sector
    for sector in 0..(size_of::<[u8; DISK_MAX_SIZE]>() / SECTOR_SIZE) {
        let mut disk = DISK.0.lock();
        let offset = sector * SECTOR_SIZE;
        read_write_disk(&mut disk[offset..offset + SECTOR_SIZE], sector as u64, false);
    }

    // Load into FILES from DISK
    let mut off = 0;
    let mut files = FILES.0.lock();
    for file in files.iter_mut() {
        let disk = DISK.0.lock();

        assert!(disk.len() >= off + size_of::<TarHeader>());
        // Safety:
        // * data is aligned to single byte alignment - not using larger types
        // * disk is initialised and valid for reading
        let header = unsafe {
            &*(disk.as_ptr().add(off) as *const TarHeader)
        };

        if header.name[0] == b'\0' { // name is a c string with nul terminator
            break;
        }

        match core::ffi::CStr::from_bytes_with_nul(&header.magic) {
            Ok(magic) if magic == c"ustar" => {},
            Ok(magic) => panic!("invalid tar header: magic={:?}", magic),
            Err(_) => panic!("invalid tar header: magic is not a valid c string"),
        }

        let filesz = oct2int(&header.size)
        .expect("file size should be valid");

        file.in_use = true;
        file.name = header.name;
        file.size = filesz;

        let data_offset = off + header.size();

        file.data[..filesz].copy_from_slice(&disk[data_offset..data_offset + filesz]);

        let file_name_str = str::from_utf8(&file.name)
        .expect("file name text should be valid UTF8")
        .trim();
        crate::println!("file: {}, size={}", file_name_str, filesz);

        off += align_up(
            header.size() + filesz,
                        SECTOR_SIZE
        );
    }
}
```
In this function, we first use the `read_write_disk` function to load the disk image into a temporary buffer (`disk` variable). The `disk` variable is declared as a static variable instead of a local (stack) variable. This is because the stack has limited size, and it's preferable to avoid using it for large data areas.

After loading the disk contents, we sequentially copy them into the `files` variable entries. Note that **the numbers in the tar header are in octal format**. It's very confusing because it looks like decimals. The `oct2int` function is used to convert these octal string values to integers.

Lastly, make sure to call the `fs_init` function after initializing the virtio-blk device (`virtio_blk_init`) in `kernel_main`:

```rust [kernel/src/main.rs]
fn kernel_main() -> ! {
    let bss = &raw const __bss;
    let bss_end = &raw const __bss_end;
    // Safety: from linker script bss is aligned and bss segment is valid for writes up to bss_end
    unsafe {
        write_bytes(bss as *mut u8, 0, bss_end as usize - bss as usize);
    }

    write_csr!("stvec", kernel_entry as usize);

    virtio_blk_init();
    fs_init();

    /* Omitted */
```

## Test file reads

Let's try! It should print the file names and their sizes in `disk` directory:

```
$ ./os1k.sh run

virtio-blk: capacity is 10240 bytes
file: hello.txt, size=84
file: meow.txt, size=6
Hello World! 🦀
> 

```

## Writing to the disk

Writing files can be implemented by writing the contents of the `files` variable back to the disk in tar file format:

```rust [kernel/src/tar.rs]
pub fn fs_flush() {
    // Copy all file contents into `disk` buffer.
    let mut disk = DISK.0.lock();
    disk.fill(0);

    let files = FILES.0.lock();
    let mut off = 0;
    for file in files.iter() {
        if !file.in_use {
            break;
        }

        // Create header
        let mut header = TarHeader::zeroed();
        header.name.copy_from_slice(&file.name);
        header.mode.copy_from_slice("00000644".as_bytes()); // Read and write permissions
        header.magic.copy_from_slice("ustar\0".as_bytes());
        header.version.copy_from_slice("00".as_bytes());
        header.typeflag = b'0'; // Regular file
        int2oct(file.size, &mut header.size);
        header.checksum.fill(b' '); // Checksum is calculated with checksum field set to spaces

        // Calculate the checksum
        let checksum = {
            // Safety: We drop buf on checksum creation to avoid mutating underlying data
            let buf = unsafe { header.as_bytes() };
            buf.iter().fold(0, | checksum, byte | checksum + *byte as usize )
        };
        int2oct(checksum, &mut header.checksum);

        // Safety: We do not mutate header in the remainder of this loop
        let buf = unsafe { header.as_bytes() };
        disk[off..off + header.size()].copy_from_slice(buf);

        // Copy file data immediately after the header.
        let data_offset = off + header.size();
        let data_size = size_of_val(&file.data);
        disk[data_offset..data_offset + data_size].copy_from_slice(&file.data);

        off += align_up(header.size() + file.size, SECTOR_SIZE);
    }

    // println!("tar: fs_flush just before write DISK is {:?}", disk);

    // Write `disk` buffer into the vitio-blk.
    for sector in 0..(DISK_MAX_SIZE / SECTOR_SIZE) {
        let offset = sector * SECTOR_SIZE;
        read_write_disk(&mut disk[offset..offset + SECTOR_SIZE], sector as u64, true);
    }

    println!("wrote {} bytes to disk", DISK_MAX_SIZE);
}
```

In this function, a tar file is built in the `disk` variable, then written to the disk using the `read_write_disk` function. Isn't it simple?

## Design file read/write system calls

Now that we have implemented file system read and write operations, let's make it possible for applications to read and write files. We'll provide two system calls: `readfile` for reading files and `writefile` for writing files. Both take as arguments the filename, a memory buffer for reading or writing, and the size of the buffer.

```rust [common/src/lib.rs]
pub const SYS_READFILE: usize = 4;
pub const SYS_WRITEFILE: usize = 5;
```

```rust [user/src/lib.rs]
pub fn readfile(filename: &str, buf: &mut [u8]) {
    let _ = sys_call(SYS_READFILE, filename.as_ptr() as isize, filename.len() as isize, buf.as_mut_ptr() as isize, buf.len() as isize);
}

pub fn writefile(filename: &str, buf: &[u8]) {
    let _ = sys_call(SYS_WRITEFILE, filename.as_ptr() as isize, filename.len() as isize,  buf.as_ptr() as isize, buf.len() as isize);
}
```

> [!TIP]
>
> It would be interesting to read the design of system calls in general operating systems and compare what has been omitted here. For example, why do `read(2)` and `write(2)` system calls in Linux take file descriptors as arguments, not filenames?

## Implement system calls

Let's implement the system calls we defined in the previous section. Let's add a file lookup method to our `struct Files`:

```rust [kernel/src/tar.rs]

impl Files {
    pub fn fs_lookup(&self, name: &str) -> Option<usize> {
        let files = self.0
            .try_borrow()
            .expect("should be able to borrow Files to get index from name");

        println!("looking up filename {}", name);

        files.iter()
            .position(|f| {  // `position` returns the index based on the closure result being true
                CStr::from_bytes_until_nul(&f.name)
                    .ok() // Converts Result<> into Option<>
                    .and_then(|cstr| cstr.to_str().ok()) // Returns None if cstr is None, otherwise calls closure
                    .is_some_and(|s| s == name) // Evaluates closure if receiving Some
        })
    }
}
```

Then add the handlers in `entry.rs`:

```rust [kernel/src/entry.rs]
fn handle_syscall(f: &mut TrapFrame) {
    let sysno = f.a4;
    match sysno {
        /* Omitted */
        SYS_READFILE | SYS_WRITEFILE => 'block: {
            let filename_ptr = f.a0 as *const u8;
            let filename_len = f.a1;

            // Safety: Caller guarantees that filename_ptr points to valid memory
            // of length filename_len that remains valid for the lifetime of this reference
            let filename = unsafe {
                str::from_utf8(slice::from_raw_parts(filename_ptr, filename_len))
            }.expect("filename must be valid UTF-8");

            let buf_ptr = f.a2 as *mut u8;
            let buf_len = f.a3;

            // Safety: Caller guarantees that buf_ptr points to valid memory
            // of length buf_len that remains valid for the lifetime of this reference
            let buf = unsafe {
                slice::from_raw_parts_mut(buf_ptr, buf_len)
            };

            // println!("handling syscall SYS_READFILE | SYS_WRITEFILE for file {:?}", filename);

            let Some(file_i) = FILES.fs_lookup(filename) else {
                println!("file not found {:x?}", filename);
                f.a0 = usize::MAX; // 2's complement is -1
                break 'block;
            };

            match sysno {
                SYS_WRITEFILE => {
                    let mut files = FILES.0.try_borrow_mut()
                        .expect("should be able to borrow FILES mutably to handle SYS_WRITEFILE");

                    files[file_i].data[..buf.len()].copy_from_slice(buf);
                    files[file_i].size = buf.len();
                    drop(files);
                    fs_flush();
                },
               SYS_READFILE => {
                    let files = FILES.0.try_borrow()
                        .expect("should be able to borrow FILES to handle SYS_READFILE");

                    buf.copy_from_slice(&files[file_i].data[..buf.len()]);
                },
                 _ => unreachable!("sysno must be SYS_READFILE or SYS_WRITEFILE"),
            }

            f.a0 = buf_len;
        },

```
File read and write operations are mostly the same, so they are grouped together in the same place. The `fs_lookup` method searches for an entry in the `FILES` variable based on the filename. For reading, it reads data from the file entry, and for writing, it modifies the contents of the file entry. Lastly, the `fs_flush` function writes to the disk.

> [!WARNING]
>
> For simplicity, we are directly referencing pointers passed from applications (aka. *user pointers*), but this poses security issues. If users can specify arbitrary memory areas, they could read and write kernel memory areas through system calls.

## File read/write commands

Let's read and write files from the shell. Since the shell doesn't implement command-line argument parsing, we'll implement `readfile` and `writefile` commands that read and write a hardcoded `hello.txt` file for now:

```rust [user/src/bin/shell.rs]
            "readfile" => {
                let mut buf = [0u8; 128];
                readfile("hello.txt", &mut buf);
                CStr::from_bytes_until_nul(&buf)
                .ok()
                .and_then(|cstr| cstr.to_str().ok())
                .map(|s| println!("{}", s.trim_end()))
                .unwrap_or_else(|| println!("could not read file contents"));
            }
            "writefile" => {
                writefile(
                    "meow.txt",
                    b"Hello from the shell!");
            },
```
It's easy peasy! However, it causes a page fault:

```
$ ./os1k.sh run

> readfile
⚠️ Panic: panicked at kernel/src/entry.rs:151:13:
unexpected trap scause=0xd, stval=0x1002005, sepc=0x80205888
```

Let's dig into the cause. According the `llvm-objdump`, it happens in a core string conversion function:

```
$ llvm-objdump -d kernel.elf
...
802057fa <_ZN4core3str8converts9from_utf817h690e91dedad74810E>:
...
80205884: 00d58733      add     a4, a1, a3
80205888: 00070703      lb      a4, 0x0(a4)

```

Upon checking the page table contents in QEMU monitor, the page at `0x1002005` (with `vaddr = 01002000`) is indeed mapped as a user page (`u`) with read, write, and execute (`rwx`) permissions:

```
QEMU 8.0.2 monitor - type 'help' for more information
(qemu) info mem
vaddr    paddr            size     attr
-------- ---------------- -------- -------
01000000 000000008029c000 00002000 rwxu-a-
01002000 000000008029e000 00010000 rwxu---

```

Let's dump the memory at the virtual address (`x` command):

```
(qemu) x /10c 0x8029e000
8029e000: 'h' 'e' 'l' 'l' '!' 'h' 'e' 'l' 'l' 'o' '.' 't' 'x' 't' 'H' 'e'
8029e010: 'l' 'l' 'o' ' '
```

If the page table settings are incorrect, the `x` command will display an error or contents in other pages. Here, we can see that the page table is correctly configured, and the pointer is indeed pointing to the string `"hello.txt"`.

In that case, what could be the cause of the page fault? The answer is:  `SUM` bit in `sstatus` CSR is not set.

## Accessing user pointers

In RISC-V, the behavior of S-Mode (kernel) can be configured through  `sstatus` CSR, including **SUM (permit Supervisor User Memory access) bit**. When SUM is not set, S-Mode programs (i.e. kernel) cannot access U-Mode (user) pages.

> [!TIP]
>
> This is a safety measure to prevent unintended references to user memory areas.
> Incidentally, Intel CPUs also have the same feature named "SMAP (Supervisor Mode Access Prevention)".

All we need to do is to set the `SUM` bit when entering user space. Define the position of the `SUM` bit as follows:

```rust [kernel/src/entry.rs] {6, 17}
// The base virtual address of an application image. This needs to match the
// starting address defined in `user.ld`.
pub const USER_BASE: usize = 0x1000000;

const SSTATUS_SPIE: usize =  1 << 5;    // Enable user mode
const SSTATUS_SUM: usize = 1 << 18;

#[unsafe(naked)]
pub extern "C" fn  user_entry() {
    naked_asm!(
        "li t0, {user_base}",
        "csrw sepc, t0",
        "li t0, {sstatus}",
        "csrw sstatus, t0",
        "sret",
        user_base = const USER_BASE,
        sstatus = const SSTATUS_SPIE | SSTATUS_SUM,
    )
}
```

> [!TIP]
>
> I explained that _"the SUM bit was the cause"_, but you may wonder how you could find this on your own. It is indeed tough - even if you are aware that a page fault is occurring, it's often hard to narrow down. Unfortunately, CPUs don't even provide detailed error codes. The reason I noticed was, simply because I knew about the SUM bit.
>
> Here are some debugging methods for when things don't work *"properly"*:
>
> - Read the RISC-V specification carefully. It does mention that *"when the SUM bit is set, S-Mode can access U-Mode pages."*
> - Read QEMU's source code. The aforementioned page fault cause is [implemented here](https://github.com/qemu/qemu/blob/d1181d29370a4318a9f11ea92065bea6bb159f83/target/riscv/cpu_helper.c#L1008). However, this can be as challenging or more so than reading the specification thoroughly.
> - Ask LLMs. Not joking. It's becoming your best pair programmer.
>
> Troubleshooting is one of the major reasons why building an OS from scratch is a time sink and OS implementers are prone to giving up. However, more you overcome these challenges, the more you'll learn and ... be super happy!

## Testing file reads/writes

Let's try reading and writing files again. `readfile` should display the contents of `hello.txt`:

```
$ ./os1k.sh run

> readfile
Can you see me? Ah, there you are! You've unlocked the achievement "Virtio Newbie!"
```

Let's also try writing to the file. Once it's done the number of bytes written should be displayed:

```
> writefile
wrote 2560 bytes to disk
```

Now the disk image has been updated with the new contents. Exit QEMU and extract `disk.tar`. You should see the updated contents:

```
$ mkdir tmp
$ cd tmp
$ tar xf ../disk.tar
$ ls -alh
total 4.0K
drwxr-xr-x  4 seiya staff 128 Jul 22 22:50 .
drwxr-xr-x 25 seiya staff 800 Jul 22 22:49 ..
-rw-r--r--  1 seiya staff  26 Jan  1  1970 hello.txt
-rw-r--r--  1 seiya staff   0 Jan  1  1970 meow.txt
$ cat hello.txt
Hello from shell!
```

You've implemented a key feature _"file system"_! Yay!
