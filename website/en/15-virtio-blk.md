# Disk I/O

In this chapter, we will implement a device driver for the virtio-blk, a virtual disk device. While virtio-blk does not exist in real hardware, it shares the very same interface as a real one.

## Virtio

Virtio is a device interface standard for virtual devices (virtio devices). In other words, it is one of the APIs for device drivers to control devices. Like you use HTTP to access web servers, you use virtio to access virtio devices. Virtio is widely used in virtualization environments such as QEMU and Firecracker.

### Virtqueue

Virtio devices have a structure called a virtqueue. As the name suggests, it is a queue shared between the driver and the device. In a nutshell:

A virtqueue consists of the following three areas:

| Name            | Written by | Content                                                                | Contents                                 |
| --------------- | ---------- | ---------------------------------------------------------------------- | ---------------------------------------------------- |
| Descriptor Area | Driver     | A table of descriptors: the address and size of the request            | Memory address, length, index of the next descriptor |
| Available Ring  | Driver     | Processing requests to the device                                      | The head index of the descriptor chain            |
| Used Ring       | Device     | Processing requests handled by the device                              | The head index of the descriptor chain            |

![virtqueue diagram](../images/virtio.svg)

Each request (e.g., a write to disk) consists of multiple descriptors, called a descriptor chain. By splitting into multiple descriptors, you can specify scattered memory data (so-called Scatter-Gather IO) or give different descriptor attributes (whether writable by the device).

For example, when writing to a disk, virtqueue will be used as follows:

1. The driver writes a read/write request in the Descriptor area.
2. The driver adds the index of the head descriptor to the Available Ring.
3. The driver notifies the device that there is a new request.
4. The device reads a request from the Available Ring and processes it.
3. The device writes the descriptor index to the Used Ring, and notifies the driver that it is complete.

For details, refer to the [virtio specification](https://docs.oasis-open.org/virtio/virtio/v1.1/csprd01/virtio-v1.1-csprd01.html). In this implementation, we will focus on a device called virtio-blk.

## Enabling virtio devices

Before writing a device driver, let's prepare a test file. Create a file named `lorem.txt` and fill it with some random text like the following:

```
$ echo "Lorem ipsum dolor sit amet, consectetur adipiscing elit. In ut magna consequat, cursus velit aliquam, scelerisque odio. Ut lorem eros, feugiat quis bibendum vitae, malesuada ac orci. Praesent eget quam non nunc fringilla cursus imperdiet non tellus. Aenean dictum lobortis turpis, non interdum leo rhoncus sed. Cras in tellus auctor, faucibus tortor ut, maximus metus. Praesent placerat ut magna non tristique. Pellentesque at nunc quis dui tempor vulputate. Vestibulum vitae massa orci. Mauris et tellus quis risus sagittis placerat. Integer lorem leo, feugiat sed molestie non, viverra a tellus." > lorem.txt
```

Also, attach a virtio-blk device to QEMU:

```bash [run.sh] {3-4}
$QEMU -machine virt -bios default -nographic -serial mon:stdio --no-reboot \
    -d unimp,guest_errors,int,cpu_reset -D qemu.log \
    -drive id=drive0,file=lorem.txt,format=raw,if=none \            # new
    -device virtio-blk-device,drive=drive0,bus=virtio-mmio-bus.0 \  # new
    -kernel kernel.elf
```

The newly added options are as follows:

- `-drive id=drive0`: Defines disk named `drive0`, with `lorem.txt` as the disk image. The disk image format is `raw` (treats the file contents as-is as disk data).
- `-device virtio-blk-device`: Adds a virtio-blk device with disk `drive0`. `bus=virtio-mmio-bus.0` maps the device into a virtio-mmio bus (virtio over Memory Mapped I/O).

## Define Rust macros/structs

First, let's add some virtio-related definitions to `virtio.rs`:

```rust [kernel/src/virtio.rs]
//! Virtio for os1k

pub const SECTOR_SIZE: usize =       512;
const VIRTQ_ENTRY_NUM: usize =       16;
const VIRTIO_DEVICE_BLK: u32 =       2;
pub const VIRTIO_BLK_PADDR: u32 = 0x10001000;
const VIRTIO_REG_MAGIC: u32 =         0x00;
const VIRTIO_REG_VERSION: u32 =       0x04;
const VIRTIO_REG_DEVICE_ID: u32 =     0x08;
const VIRTIO_REG_QUEUE_SEL: u32 =     0x30;
#[expect(dead_code)]
const VIRTIO_REG_QUEUE_NUM_MAX: u32 = 0x34;
const VIRTIO_REG_QUEUE_NUM: u32 =     0x38;
const VIRTIO_REG_QUEUE_ALIGN: u32 =   0x3c;
const VIRTIO_REG_QUEUE_PFN: u32 =     0x40;
#[expect(dead_code)]
const VIRTIO_REG_QUEUE_READY: u32 =   0x44;
const VIRTIO_REG_QUEUE_NOTIFY: u32 =  0x50;
const VIRTIO_REG_DEVICE_STATUS: u32 = 0x70;
const VIRTIO_REG_DEVICE_CONFIG: u32 = 0x100;
const VIRTIO_STATUS_ACK: u32 =       1;
const VIRTIO_STATUS_DRIVER: u32 =    2;
const VIRTIO_STATUS_DRIVER_OK: u32 = 4;
const VIRTIO_STATUS_FEAT_OK: u32 =   8;
const VIRTQ_DESC_F_NEXT: u32 =          1;
const VIRTQ_DESC_F_WRITE: u32 =         2;
#[expect(dead_code)]
const VIRTQ_AVAIL_F_NO_INTERRUPT: u32 = 1;
const VIRTIO_BLK_T_IN: u32 =  0;
const VIRTIO_BLK_T_OUT: u32 = 1;

// Virtqueue Descriptor area entry.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct VirtqDesc {
    addr:u64,
    len: u32,
    flags: u16,
    next: u16,
}

// Virtqueue Available Ring.
#[repr(C, packed)]
#[derive(Debug)]
struct VirtqAvail {
    flags: u16,
    index: u16,
    ring: [u16; VIRTQ_ENTRY_NUM],
}

// Virtqueue Used Ring entry.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct VirtqUsedElem {
    id: u32,
    len: u32,
}

// Virtqueue Used Ring.
#[repr(C, packed)]
#[derive(Debug)]
struct VirtqUsed {
    flags: u16,
    index: u16,
    ring: [VirtqUsedElem; VIRTQ_ENTRY_NUM],
}

// Page-aligned VirtqUsed
#[repr(C, align(4096))]
#[derive(Debug)]
struct AlignedVirtqUsed(VirtqUsed);

// Virtqueue.
#[repr(C)]  // Not packed, as VirtqUsed is aligned to page size
#[derive(Debug)]
struct VirtioVirtq {
    descs: [VirtqDesc; VIRTQ_ENTRY_NUM],
    avail: VirtqAvail,
    used: AlignedVirtqUsed,  // Needs align to page size
    queue_index: u16,
    used_index: *mut u16, // Only access using ptr::read_volatile
    last_used_index: u16,
}

impl VirtioVirtq {
    fn zeroed() -> Self {
        // SAFETY: VirtioVirtq contains only structs/arrays of integers and pointers.
        // All-zero bytes is a valid representation: integers become 0, pointer becomes null.
        unsafe { core::mem::MaybeUninit::zeroed().assume_init() }
    }
}

// Safety: Single threaded OS
unsafe impl Sync for VirtioVirtq {}
// SAFETY: VirtioVirtq contains a pointer to memory-mapped I/O registers.
// This pointer is only accessed while holding the SpinLock, ensuring
// no concurrent access occurs. The hardware is accessible from any CPU core.
unsafe impl Send for VirtioVirtq {}

// Virtio-blk request.
#[repr(C, packed)]
#[derive(Debug)]
struct VirtioBlkReq {
    req_type: u32,
    reserved: u32,
    sector: u64,
    data: [u8; 512],
    status: u8,
}

//Safety: Single threaded OS
unsafe impl Sync for VirtioBlkReq {}

impl VirtioBlkReq {
    fn zeroed() -> Self {
        // SAFETY: VirtioBlkReq is a packed C struct with only integer/array fields.
        // All-zero bytes is a valid representation for this type.
        unsafe { core::mem::MaybeUninit::zeroed().assume_init() }
    }
}
```
> [!NOTE]
>
> `#[repr(C, packed)]` is a compiler directive that tells the compiler to pack the struct members without *padding* and keep them in the same order as specified. Otherwise, the compiler may add hidden padding bytes and driver/device may see different values, or may swap the order of struct members.

Next, add utility functions to `virtio` for accessing MMIO registers:

```rust [kernel/src/virtio.rs]
fn virtio_reg_read32(offset: u32) -> u32 {
    // Safety:
    // * VIRTIO_BLK_PADDR + offset is valid for reads
    // * VIRTIO_BLK_PADDR is 32-bit aligned and offset is 32-bit aligned
    // * VIRTIO_BLK_PADDR + offset points to a QEMU initialized `u32`
    // * `u32` is Copy
    assert_eq!((VIRTIO_BLK_PADDR + offset) % align_of::<u32>() as u32, 0);
    unsafe {
        ptr::read_volatile((VIRTIO_BLK_PADDR + offset) as *const u32)
    }
}

fn virtio_reg_read64(offset: u32) -> u64 {
    // Safety:
    // * VIRTIO_BLK_PADDR + offset is valid for reads
    // * VIRTIO_BLK_PADDR is 64-bit aligned and offset is 64-bit aligned
    // * VIRTIO_BLK_PADDR + offset points to a QEMU initialized `u64`
    // * `u64` is Copy
    assert_eq!((VIRTIO_BLK_PADDR + offset) % align_of::<u64>() as u32, 0);
    unsafe {
        ptr::read_volatile((VIRTIO_BLK_PADDR + offset) as *const u64)
    }
}

fn virtio_reg_write32(offset: u32, value: u32) {
    // Safety:
    // * VIRTIO_BLK_PADDR + offset is valid for writes.
    // * VIRTIO_BLK_PADDR + offset is properly 32-bit aligned.
    assert_eq!((VIRTIO_BLK_PADDR + offset) % align_of::<u32>() as u32, 0);
    unsafe {
        ptr::write_volatile((VIRTIO_BLK_PADDR + offset) as *mut u32, value)
    }
}

fn virtio_reg_fetch_and_or32(offset: u32, value: u32) {
    // Safety:
    // * Caller ensures VIRTIO_BLK_PADDR + offset is valid for reads and writes
    virtio_reg_write32(offset, virtio_reg_read32(offset) | value);
}
```
> [!WARNING]
>
> Accessing MMIO registers are not same as accessing normal memory. You should use `ptr::read_volatile` or `ptr::write_volatile` to prevent the compiler from optimizing out the read/write operations. In MMIO, memory access may trigger side effects (e.g., sending a command to the device).

## Map the MMIO region

First, map the `virtio-blk` MMIO region to the page table so that the kernel can access the MMIO registers. We do this in `process`. It's super simple:

```rust [kernel/src/process.rs] {9}
pub fn create_process(image: *const u8, image_size: usize) -> usize {
    /* omitted */

    for paddr in (kernel_base..free_ram_end).step_by(PAGE_SIZE) {
        map_page(page_table.as_mut(), VAddr::new(paddr), PAddr::new(paddr), PAGE_R | PAGE_W | PAGE_X);
    }

    map_page(page_table.as_mut(), VAddr::new(VIRTIO_BLK_PADDR as usize), PAddr::new(VIRTIO_BLK_PADDR as usize), PAGE_R | PAGE_W); // new
```

## Virtio device initialization

The initialization process is detailed in the [virtio specification](https://docs.oasis-open.org/virtio/virtio/v1.1/csprd01/virtio-v1.1-csprd01.html#x1-910003):

> 3.1.1 Driver Requirements: Device Initialization
> The driver MUST follow this sequence to initialize a device:
>
> 1. Reset the device.
> 2. Set the ACKNOWLEDGE status bit: the guest OS has noticed the device.
> 3. Set the DRIVER status bit: the guest OS knows how to drive the device.
> 4. Read device feature bits, and write the subset of feature bits understood by the OS and driver to the device. During this step the driver MAY read (but MUST NOT write) the device-specific configuration fields to check that it can support the device before accepting it.
> 5. Set the FEATURES_OK status bit. The driver MUST NOT accept new feature bits after this step.
> 6. Re-read device status to ensure the FEATURES_OK bit is still set: otherwise, the device does not support our subset of features and the device is unusable.
> 7. Perform device-specific setup, including discovery of virtqueues for the device, optional per-bus setup, reading and possibly writing the device’s virtio configuration space, and population of virtqueues.
> 8. Set the DRIVER_OK status bit. At this point the device is “live”.

You might be overwhelmed by lengthy steps, but don't worry. A naive implementation is very simple:

```rust [kernel/src/virtio.rs]
static BLK_REQUEST_VQ: SpinLock<Option<Box<VirtioVirtq>>> = SpinLock::new(None);
static BLK_REQ: SpinLock<Option<Box<VirtioBlkReq>>> = SpinLock::new(None);
static BLK_CAPACITY: SpinLock<Option<u64>> = SpinLock::new(None);

pub fn virtio_blk_init() {
    if virtio_reg_read32(VIRTIO_REG_MAGIC) != 0x74726976 {
        panic!("virtio: invalid magic value");
    };
    if virtio_reg_read32(VIRTIO_REG_VERSION) != 1 {
        panic!("virtio: invalid version");
    };

    if virtio_reg_read32(VIRTIO_REG_DEVICE_ID) != VIRTIO_DEVICE_BLK {
        panic!("virtio: invalid version");
    };

    // 1. Reset the device
    virtio_reg_write32(VIRTIO_REG_DEVICE_STATUS, 0);
    // 2. Set the ACKNOWLEDGE status bit: the guest OS has noticed the device
    virtio_reg_fetch_and_or32(VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_ACK);
    // 3. Set the DRIVER status bit.
    virtio_reg_fetch_and_or32(VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_DRIVER);
    // 5. Set the FEATURES_OK status bit
    virtio_reg_fetch_and_or32(VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_FEAT_OK);
    // 7. Perform device-specific setup, including discovery of virtqueues for the device
    *BLK_REQUEST_VQ.lock() = Some(virtq_init(0));
    // 8. Set the DRIVER_OK status bit.
    virtio_reg_write32(VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_DRIVER_OK);

    // Get the disk capacity.
    *BLK_CAPACITY.lock() = Some(virtio_reg_read64(VIRTIO_REG_DEVICE_CONFIG + 0) * SECTOR_SIZE as u64);

    match *BLK_CAPACITY.lock() {
        Some(capacity) => println!("virtio-blk: capacity is {} bytes", capacity),
        None => println!("virtio-blk: capacity is not initialized yet"),
    }

    // Allocate a region to store requests to the device.
    *BLK_REQ.lock() = Some(Box::new(VirtioBlkReq::zeroed()));
}
```

We also need to initialise this in our main procedure in `main`.

```rust [kernel/src/main.rs] {11}
fn kernel_main() -> ! {
    let bss = &raw const __bss;
    let bss_end = &raw const __bss_end;
    // Safety: from linker script bss is aligned and bss segment is valid for writes up to bss_end
    unsafe {
        write_bytes(bss as *mut u8, 0, bss_end as usize - bss as usize);
    }

    write_csr!("stvec", kernel_entry as usize);

    virtio_blk_init();
```

## Virtqueue initialization

Virtqueues also need to be initialized. Let's read the specification:

> The virtual queue is configured as follows:
>
> 1. Select the queue writing its index (first queue is 0) to QueueSel.
> 2. Check if the queue is not already in use: read QueuePFN, expecting a returned value of zero (0x0).
> 3. Read maximum queue size (number of elements) from QueueNumMax. If the returned value is zero (0x0) the queue is not available.
> 4. Allocate and zero the queue pages in contiguous virtual memory, aligning the Used Ring to an optimal boundary (usually page size). The driver should choose a queue size smaller than or equal to QueueNumMax.
> 5. Notify the device about the queue size by writing the size to QueueNum.
> 6. Notify the device about the used alignment by writing its value in bytes to QueueAlign.
> 7. Write the physical number of the first page of the queue to the QueuePFN register.

Here's a simple implementation:

```rust [kernel/src/virtio.rs]
fn virtq_init(index: usize) ->  Box<VirtioVirtq> {
    // Allocate a region for the virtqueue.
    let mut vq = Box::new(VirtioVirtq::zeroed());

    vq.queue_index = index as u16;
    vq.used_index = &raw mut vq.used.0.index; // Create pointer for read_volatile

    // 1. Select the queue writing its index (first queue is 0) to QueueSel.
    virtio_reg_write32(VIRTIO_REG_QUEUE_SEL, index as u32);
    // 5. Notify the device about the queue size by writing the size to QueueNum.
    virtio_reg_write32(VIRTIO_REG_QUEUE_NUM, VIRTQ_ENTRY_NUM as u32);
    // 6. Notify the device about the used alignment by writing its value in bytes to QueueAlign.
    virtio_reg_write32(VIRTIO_REG_QUEUE_ALIGN, 0);
    // 7. Write the physical number of the first page of the queue to the QueuePFN register.
    virtio_reg_write32(VIRTIO_REG_QUEUE_PFN, &*vq as * const _ as u32); // In our OS the virtual address matches the physical address

    vq
}
```
This function allocates a memory region for a virtqueue, and tells the its physical address to the device. The device will use this memory region to read/write requests.

> [!TIP]
>
> What drivers do in the initialization process is to check device capabilities/features, allocating OS resources (e.g., memory regions), and setting parameters. Isn't it similar to handshakes in network protocols?

## Sending I/O requests

We now have an initialized virtio-blk device. Let's send an I/O request to the disk. I/O requests to the disk is implemented by _"adding processing requests to the virtqueue"_ as follows:

```rust [kernel/src/virtio.rs]
// Notifies the device that there is a new request. `desc_index` is the index of the head descriptor of the new request
fn virtq_kick(vq: &mut VirtioVirtq, desc_index: u16) {
    let index = vq.avail.index as usize % VIRTQ_ENTRY_NUM;
    vq.avail.ring[index] = desc_index;
    vq.avail.index += 1;

    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst); // Equivalent to __sync_synchronise();

    virtio_reg_write32(VIRTIO_REG_QUEUE_NOTIFY, vq.queue_index.into());  // converting `u16` to `u32` cannot fail
    vq.last_used_index += 1;
}

// Returns whether there are requests being processed by the device.
fn virtq_is_busy(vq: &VirtioVirtq) -> bool {
    // Safety:
    // * vq.used_index is valid for reads
    // * vq.used_index is 16-bit aligned
    // * vq.used_index points to a value properly initialised by QEMU
    // * `u16` is Copy
    assert_eq!(vq.used_index as usize % align_of::<u16>(), 0);
    unsafe {
        vq.last_used_index != core::ptr::read_volatile(vq.used_index)
    }
}

// Reads/writes from/to virtio-blk device.
pub fn read_write_disk(buf: &mut [u8], sector: u64, is_write: bool) {
    let blk_capacity = BLK_CAPACITY.lock()
        .expect("block capacity initialised before read_write_disk call.");
    if sector >= (blk_capacity / SECTOR_SIZE as u64) {
        println!("virtio: tried to read/write sector={}, but capacity is {}", sector, blk_capacity / SECTOR_SIZE as u64);
        return;
    }

    let mut br_guard = BLK_REQ.lock();
    let br = br_guard.as_mut()
        .expect("BLK_REQ not initialised");

    br.sector = sector;
    br.req_type = if is_write { VIRTIO_BLK_T_OUT } else { VIRTIO_BLK_T_IN };

    if is_write {
        br.data.copy_from_slice(buf);
    };

    // Construct the virtqueue descriptors (using 3 descriptors).
    let mut vq_guard = BLK_REQUEST_VQ.lock();
    let vq = vq_guard.as_mut().expect("BLK_REQUEST_VQ not initialised");

    let blk_req_paddr = &**br as *const VirtioBlkReq as usize; // Double deference to get address from heap, not of the Box

    // Descriptor 0: request header
    vq.descs[0] = VirtqDesc {
        addr: blk_req_paddr as u64,
        len: (mem::size_of::<u32>() * 2 + mem::size_of::<u64>()) as u32,
        flags: VIRTQ_DESC_F_NEXT as u16,
        next: 1,
    };

    // Descriptor 1: data buffer
    vq.descs[1] = VirtqDesc {
        addr: (blk_req_paddr + offset_of!(VirtioBlkReq, data)) as u64,
        len: SECTOR_SIZE as u32,
        flags: (VIRTQ_DESC_F_NEXT | (if is_write {0} else {VIRTQ_DESC_F_WRITE})) as u16,
        next: 2,
    };

    // Descriptor 2: status byte
    vq.descs[2] = VirtqDesc {
        addr: (blk_req_paddr + offset_of!(VirtioBlkReq, status)) as u64,
        len: mem::size_of::<u8>() as u32,
        flags: VIRTQ_DESC_F_WRITE as u16,
        next: 0,
    };

    // Notify the device that there is a new request.
    virtq_kick(vq.as_mut(), 0);

    // Wait until the device finishes processing.
    while virtq_is_busy(vq.as_ref()) {
        core::hint::spin_loop();
    }

    // virtio-blk: If a non-zero value is returned, it's an error.
    if br.status != 0 {
        println!("virtio: warn: failed to read/write sector={} status={}", sector, br.status);
        return;
    }

    // For read operations, copy the data into the buffer.
    if !is_write {
        buf.copy_from_slice(&br.data);
    }
}
```
A request is sent in the following steps:

1. Construct a request in `blk_req`. Specify the sector number you want to access and the type of read/write.
2. Construct a descriptor chain pointing to each area of `blk_req` (see below).
3. Add the index of the head descriptor of the descriptor chain to the Available Ring.
4. Notify the device that there is a new pending request.
5. Wait until the device finishes processing (aka *busy-waiting* or *polling*).
6. Check the response from the device.

Here, we construct a descriptor chain consisting of 3 descriptors. We need 3 descriptors because each descriptor has different attributes (`flags`) as follows:

```rust
struct VirtioBlkReq {
    // First descriptor: read-only from the device
    req_type: u32,
    reserved: u32,
    sector: u64,

     // Second descriptor: writable by the device if it's a read operation 
    data: [u8; 512],

    // Third descriptor: writable by the device (VIRTQ_DESC_F_WRITE)
    status: u8,
}
```
Because we busy-wait until the processing is complete every time, we can simply use the *first* 3 descriptors in the ring. However, in practice, you need to track free/used descriptors to process multiple requests simultaneously.

## Try it out

Lastly, let's try disk I/O. Add the following code to `main.rs`:

```rust [kernel/src/main.rs] {3-10}
    virtio_blk_init();

    let mut buf: [u8; SECTOR_SIZE] = [0u8; SECTOR_SIZE];
    read_write_disk(&mut buf, 0, false /* read from the disk */);
    print!("first sector:");
    for &b in &buf {
        let _ = crate::sbi::put_byte(b);
    }
    let s = "hello from kernel!!!";
    buf[..s.len()].copy_from_slice(s.as_bytes());
    read_write_disk(&mut buf, 0, true /* write to the disk */);
```
Since we specify `lorem.txt` as the (raw) disk image, its contents should be displayed as-is:

```
$ ./os1k.sh

virtio-blk: capacity is 1024 bytes
first sector: Lorem ipsum dolor sit amet, consectetur adipiscing elit ...
```

Also, the first sector is overwritten with the string "hello from kernel!!!":

```
$ head lorem.txt
hello from kernel!!!t amet, consectetur adipiscing elit. In ...
```

Congratulations! You've successfully implemented a disk I/O driver!

> [!TIP]
> As you might notice, device drivers are just "glue" between the OS and devices. Drivers don't control the hardware directly; drivers communicate with other software running on the device (e.g., firmware). Devices and their software, not the OS driver, will do the rest of the heavy lifting, like moving disk read/write heads.
