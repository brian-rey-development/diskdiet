use super::Node;
use rustix::fs::{Mode, OFlags};
use std::cell::RefCell;
use std::ffi::OsStr;
use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

const BUFFER_BYTES: usize = 128 * 1024;
const DIRECTORY_OBJTYPE: u32 = 2;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const VOLUME_AND_DIR_SETS: usize = 2 * U32_BYTES;
const REQUIRED_COMMON: u32 =
    libc::ATTR_CMN_NAME | libc::ATTR_CMN_DEVID | libc::ATTR_CMN_OBJTYPE | libc::ATTR_CMN_FILEID;
const REQUESTED_COMMON: u32 = libc::ATTR_CMN_RETURNED_ATTRS | REQUIRED_COMMON;
const REQUIRED_FILE: u32 = libc::ATTR_FILE_LINKCOUNT | libc::ATTR_FILE_ALLOCSIZE;

thread_local! {
    static BUFFER: RefCell<Vec<u8>> = RefCell::new(vec![0; BUFFER_BYTES]);
}

fn open_dir(dir: &Path) -> io::Result<OwnedFd> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    Ok(rustix::fs::open(dir, flags, Mode::empty())?)
}

fn request() -> libc::attrlist {
    libc::attrlist {
        bitmapcount: libc::ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: REQUESTED_COMMON,
        volattr: 0,
        dirattr: 0,
        fileattr: REQUIRED_FILE,
        forkattr: 0,
    }
}

fn fill(dir: &OwnedFd, buffer: &mut [u8]) -> io::Result<usize> {
    let mut attrs = request();
    // SAFETY: attrs and buffer are live, writable and buffer.len() is its real size.
    let count = unsafe {
        libc::getattrlistbulk(
            dir.as_raw_fd(),
            (&raw mut attrs).cast(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            0,
        )
    };
    usize::try_from(count).map_err(|_| io::Error::last_os_error())
}

pub(super) fn list(dir: &Path) -> io::Result<Vec<Node>> {
    let handle = open_dir(dir)?;
    let mut nodes = Vec::new();
    BUFFER.with_borrow_mut(|buffer| {
        loop {
            let count = fill(&handle, buffer)?;
            if count == 0 {
                return Ok(nodes);
            }
            parse_batch(buffer, count, dir, &mut nodes).ok_or_else(unsupported)?;
        }
    })
}

fn unsupported() -> io::Error {
    io::Error::from(io::ErrorKind::Unsupported)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let end = self.at.checked_add(N)?;
        let chunk = self.bytes.get(self.at..end)?.try_into().ok()?;
        self.at = end;
        Some(chunk)
    }

    fn u32(&mut self) -> Option<u32> {
        self.take::<U32_BYTES>().map(u32::from_ne_bytes)
    }

    fn i32(&mut self) -> Option<i32> {
        self.take::<U32_BYTES>().map(i32::from_ne_bytes)
    }

    fn u64(&mut self) -> Option<u64> {
        self.take::<U64_BYTES>().map(u64::from_ne_bytes)
    }

    fn i64(&mut self) -> Option<i64> {
        self.take::<U64_BYTES>().map(i64::from_ne_bytes)
    }

    fn skip(&mut self, bytes: usize) -> Option<()> {
        self.at = self.at.checked_add(bytes)?;
        Some(())
    }
}

fn parse_batch(buffer: &[u8], count: usize, dir: &Path, nodes: &mut Vec<Node>) -> Option<()> {
    let mut at = 0;
    for _ in 0..count {
        let length = Cursor { bytes: buffer, at }.u32()? as usize;
        let end = at.checked_add(length)?;
        nodes.push(parse_entry(buffer.get(at..end)?, dir)?);
        at = end;
    }
    Some(())
}

fn parse_name<'a>(cursor: &mut Cursor<'a>) -> Option<&'a [u8]> {
    let base = cursor.at;
    let offset = isize::try_from(cursor.i32()?).ok()?;
    let length = cursor.u32()? as usize;
    let start = base.checked_add_signed(offset)?;
    let raw = cursor.bytes.get(start..start.checked_add(length)?)?;
    Some(raw.strip_suffix(&[0]).unwrap_or(raw))
}

fn parse_entry(entry: &[u8], dir: &Path) -> Option<Node> {
    let mut cursor = Cursor {
        bytes: entry,
        at: U32_BYTES,
    };
    let common = cursor.u32()?;
    cursor.skip(VOLUME_AND_DIR_SETS)?;
    let file = cursor.u32()?;
    cursor.skip(U32_BYTES)?;
    if common & REQUIRED_COMMON != REQUIRED_COMMON {
        return None;
    }
    let name = parse_name(&mut cursor)?;
    let device = cursor.i32()?;
    let objtype = cursor.u32()?;
    let inode = cursor.u64()?;
    if objtype == DIRECTORY_OBJTYPE {
        return Some(Node::Dir(dir.join(OsStr::from_bytes(name)), 0));
    }
    parse_file(&mut cursor, file, device, inode)
}

fn parse_file(cursor: &mut Cursor, file: u32, device: i32, inode: u64) -> Option<Node> {
    if file & REQUIRED_FILE != REQUIRED_FILE {
        return None;
    }
    let links = cursor.u32()?;
    let allocated = cursor.i64()?;
    Some(Node::File {
        bytes: u64::try_from(allocated).unwrap_or(0),
        shared: (links > 1).then_some((i64::from(device).cast_unsigned(), inode)),
    })
}
