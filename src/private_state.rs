//! Private persisted state preparation; operator-owned parents remain trusted.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::SinkError;

fn unavailable<T>(_: T) -> SinkError {
    SinkError::Unavailable
}

fn reject() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, "unsafe private state path")
}

fn resolved(path: &Path) -> io::Result<PathBuf> {
    let name = path.file_name().ok_or_else(reject)?;
    let text = name.to_string_lossy();
    if text == ":memory:" || text.starts_with("file:") {
        return Err(reject());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(parent)?;
    let parent = fs::canonicalize(parent)?;
    validate_parent(&parent)?;
    validate_acl(&parent)?;
    Ok(parent.join(name))
}

#[cfg(unix)]
fn validate_parent(parent: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid has no preconditions or pointer arguments.
    let owner = unsafe { libc::geteuid() };
    let metadata = fs::metadata(parent)?;
    if !metadata.is_dir() || metadata.uid() != owner || metadata.mode() & 0o022 != 0 {
        return Err(reject());
    }
    // Sticky system temp directories may hold an owned private state directory.
    // Other writable ancestors could allow replacement of that directory.
    for ancestor in parent.ancestors().skip(1) {
        let metadata = fs::metadata(ancestor)?;
        if metadata.mode() & 0o022 != 0 && metadata.mode() & 0o1000 == 0 {
            return Err(reject());
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_parent(_: &Path) -> io::Result<()> {
    Ok(())
}

fn validate_file(metadata: &fs::Metadata) -> io::Result<()> {
    if !metadata.file_type().is_file() {
        return Err(reject());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: geteuid has no preconditions or pointer arguments.
        let owner = unsafe { libc::geteuid() };
        if metadata.uid() != owner || metadata.mode() & 0o7777 != 0o600 || metadata.nlink() != 1 {
            return Err(reject());
        }
    }
    Ok(())
}

fn existing(path: &Path) -> io::Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            validate_file(&metadata)?;
            validate_acl(path)?;
            Ok(Some(metadata))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn options() -> OpenOptions {
    #[cfg(unix)]
    let mut options = OpenOptions::new();
    #[cfg(not(unix))]
    let options = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    options
}

fn same_file(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        before.dev() == after.dev() && before.ino() == after.ino()
    }
    #[cfg(not(unix))]
    {
        let _ = (before, after);
        true
    }
}

fn prepare_db(path: &Path) -> io::Result<fs::Metadata> {
    if let Some(metadata) = existing(path)? {
        return Ok(metadata);
    }
    match options().read(true).write(true).create_new(true).open(path) {
        Ok(file) => {
            let metadata = file.metadata()?;
            validate_file(&metadata)?;
            validate_acl(path)?;
            // Close only the new-file descriptor, before SQLite holds any lock.
            Ok(metadata)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            existing(path)?.ok_or_else(reject)
        }
        Err(error) => Err(error),
    }
}

fn append_file(path: &Path) -> io::Result<File> {
    let file = match options().append(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            options().append(true).open(path)?
        }
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    validate_file(&metadata)?;
    let current = existing(path)?.ok_or_else(reject)?;
    if !same_file(&metadata, &current) {
        return Err(reject());
    }
    Ok(file)
}

fn distinct_copies(db: &Path, events: &Path, sidecars: &[PathBuf]) -> io::Result<()> {
    if let Some(events_metadata) = existing(events)? {
        for copy in std::iter::once(db).chain(sidecars.iter().map(PathBuf::as_path)) {
            if let Some(copy_metadata) = existing(copy)? {
                #[cfg(unix)]
                let aliases = same_file(&events_metadata, &copy_metadata);
                #[cfg(windows)]
                let aliases = windows_identity(events)? == windows_identity(copy)?;
                #[cfg(not(any(unix, windows)))]
                let aliases = fs::canonicalize(events)? == fs::canonicalize(copy)?;
                #[cfg(not(unix))]
                let _ = (&events_metadata, &copy_metadata);
                if aliases {
                    return Err(reject());
                }
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn windows_identity(path: &Path) -> io::Result<(u32, u32, u32)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let file = File::open(path)?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: the live File owns the handle, and info is a valid output buffer.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((
        info.dwVolumeSerialNumber,
        info.nFileIndexHigh,
        info.nFileIndexLow,
    ))
}

#[cfg(target_os = "macos")]
fn validate_acl(path: &Path) -> io::Result<()> {
    use std::ffi::{c_char, c_int, c_void, CString};
    use std::os::unix::ffi::OsStrExt;
    // ABI/constants from macOS sys/acl.h; no subprocess or state-file descriptor.
    extern "C" {
        fn acl_get_file(path: *const c_char, kind: c_int) -> *mut c_void;
        fn acl_get_entry(acl: *mut c_void, index: c_int, entry: *mut *mut c_void) -> c_int;
        fn acl_valid(acl: *mut c_void) -> c_int;
        fn acl_free(acl: *mut c_void) -> c_int;
    }
    let name = CString::new(path.as_os_str().as_bytes()).map_err(|_| reject())?;
    // SAFETY: name is NUL-terminated; an owned returned ACL is freed once below.
    let acl = unsafe { acl_get_file(name.as_ptr(), 0x100) };
    if acl.is_null() {
        let error = io::Error::last_os_error();
        // Darwin reports ENOENT for an existing object without an extended ACL.
        // Distinguish that representation from disappearance of the object.
        if error.raw_os_error() == Some(libc::ENOENT) {
            return fs::symlink_metadata(path).map(|_| ());
        }
        if error.raw_os_error() == Some(libc::ENOTSUP) {
            return Ok(());
        }
        return Err(error);
    }
    let mut entry = std::ptr::null_mut();
    // SAFETY: acl is live; entry is a valid output pointer. FIRST_ENTRY is 0.
    let valid = unsafe { acl_valid(acl) } == 0;
    let result = unsafe { acl_get_entry(acl, 0, &mut entry) };
    let empty = result == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::EINVAL);
    // SAFETY: acl_get_file returned this owned object, not freed elsewhere.
    unsafe { acl_free(acl) };
    if valid && empty {
        Ok(())
    } else {
        Err(reject())
    }
}

#[cfg(target_os = "linux")]
fn validate_acl(path: &Path) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let name = CString::new(path.as_os_str().as_bytes()).map_err(|_| reject())?;
    for attribute in [
        b"system.posix_acl_access\0".as_slice(),
        b"system.posix_acl_default\0".as_slice(),
    ] {
        // SAFETY: names are NUL-terminated and null/zero requests only the size.
        let size = unsafe {
            libc::getxattr(
                name.as_ptr(),
                attribute.as_ptr().cast(),
                std::ptr::null_mut(),
                0,
            )
        };
        if size >= 0 {
            return Err(reject());
        }
        match io::Error::last_os_error().raw_os_error() {
            Some(libc::ENODATA) | Some(libc::ENOTSUP) => {}
            _ => return Err(reject()),
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn validate_acl(_: &Path) -> io::Result<()> {
    Ok(())
}

pub(crate) fn open(db: &Path, events: &Path) -> Result<(Connection, File), SinkError> {
    let db = resolved(db).map_err(unavailable)?;
    let events = resolved(events).map_err(unavailable)?;
    let sidecars: Vec<_> = ["-wal", "-shm", "-journal"]
        .into_iter()
        .map(|suffix| {
            let mut name = db.as_os_str().to_os_string();
            name.push(suffix);
            PathBuf::from(name)
        })
        .collect();
    if db == events || sidecars.contains(&events) {
        return Err(SinkError::Unavailable);
    }
    // Validate every existing copy before SQLite can recover or modify it.
    for path in std::iter::once(&db)
        .chain(std::iter::once(&events))
        .chain(sidecars.iter())
    {
        existing(path).map_err(unavailable)?;
    }
    let identity = prepare_db(&db).map_err(unavailable)?;
    // Filesystem aliases include case folding/Unicode normalization. Reject an
    // existing alias before opening another descriptor to a live SQLite inode.
    distinct_copies(&db, &events, &sidecars).map_err(unavailable)?;
    let events_file = append_file(&events).map_err(unavailable)?;
    // Creating JSONL may also create an aliased, previously absent sidecar.
    distinct_copies(&db, &events, &sidecars).map_err(unavailable)?;
    // No CREATE or URI: precreation defines the real persisted file and mode.
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_NOFOLLOW;
    let conn = Connection::open_with_flags(&db, flags).map_err(unavailable)?;
    let current = existing(&db)
        .map_err(unavailable)?
        .ok_or(SinkError::Unavailable)?;
    if !same_file(&identity, &current) {
        return Err(SinkError::Unavailable);
    }
    Ok((conn, events_file))
}
