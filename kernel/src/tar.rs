//! Tar as a file system

use core::ffi::CStr;
use core::fmt::Debug;

use common::println;

use crate::address::align_up;
use crate::spinlock::SpinLock;
use crate::virtio::{read_write_disk, SECTOR_SIZE};

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
    // const fn empty() -> Self {
    //     Self {
    //         name: [0u8; 100],
    //         mode: [0u8; 8],
    //         uid: [0u8; 8],
    //         gid: [0u8; 8],
    //         size: [0u8; 12],
    //         mtime: [0u8; 12],
    //         checksum: [0u8; 8],
    //         typeflag: 0u8,
    //         linkname: [0u8; 100],
    //         magic: [0u8; 6],
    //         version: [0u8; 2],
    //         uname: [0u8; 32],
    //         gname: [0u8; 32],
    //         devmajor: [0u8; 8],
    //         devminor: [0u8; 8],
    //         prefix: [0u8; 155],
    //         _padding: [0u8; 12],
    //     }
    // }

    fn zeroed() -> Self {
        // SAFETY: VirtioVirtq contains only structs/arrays of integers and pointers.
        // All-zero bytes is a valid representation: integers become 0, pointer becomes null.
        unsafe { core::mem::MaybeUninit::zeroed().assume_init() }
    }

    /// SAFETY: It is UB to mutate the underlying memory while this byte array exists
    unsafe fn as_bytes(&self) -> &[u8] {
        // Safety:
        // * self pointer is non_null as this can only be called on an existing struct,
        // * self is valid for reads for entire struct
        // * self is properly aligned to 1 byte for u8
        // * the entire memory range is within size_of::<Self>()
        // * header pointer points to size_of::<Self>() properly initialised bytes thanks to empty()
        // * we do not mutate the underlying memory of the slice once header is created
        // * total size of TarHeader is smaller than isize::MAX
        unsafe {
            core::slice::from_raw_parts(
                self as *const Self as *const u8,
                self.size())
        }
    }

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
    // const fn empty() -> Self {
    //     Self {
    //         in_use: false,
    //         name: [0u8; 100],
    //         data: [0u8; 1024],
    //         size: 0,
    //     }
    // }

    const fn zeroed() -> Self {
        // SAFETY: VirtioVirtq contains only structs/arrays of integers and pointers.
        // All-zero bytes is a valid representation: integers become 0, pointer becomes null.
        unsafe { core::mem::MaybeUninit::zeroed().assume_init() }
    }
}

// #[derive(Clone, Debug)]
// pub struct Files ( pub RefCell<[File; FILES_MAX]>);

#[derive(Debug)]
pub struct Files(pub SpinLock<[File; FILES_MAX]>);

//Safety: Single threaded OS
unsafe impl Sync for Files {}

impl Files {
    pub fn fs_lookup(&self, name: &str) -> Option<usize> {
        let files = self.0.lock();

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

// pub static FILES: Files = Files(RefCell::new([File::empty(); FILES_MAX]));

pub static FILES: Files = Files(SpinLock::new([File::zeroed(); FILES_MAX]));

// #[derive(Debug)]
// pub struct Disk(RefCell<[u8; DISK_MAX_SIZE]>);

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
        // let ptr = &raw mut disk[sector * SECTOR_SIZE];
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

    // println!("at the end of fs_init, FILES is {:?}", FILES);
}

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
