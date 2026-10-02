#[cfg(windows)]
use std::{fs, path::Path};

#[cfg(windows)]
use crate::Result;

#[cfg(windows)]
pub(crate) fn windows_create_new(path: &Path) -> Result<fs::File> {
  use std::{
    io,
    os::windows::{ffi::OsStrExt, io::FromRawHandle},
  };

  use windows_sys::Win32::{
    Foundation::{GENERIC_WRITE, INVALID_HANDLE_VALUE, LocalFree},
    Security::{
      Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW, SECURITY_ATTRIBUTES,
    },
    Storage::FileSystem::{CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_NORMAL},
  };
  let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
  if path[.. path.len() - 1].contains(&0) {
    return Err(crate::Error::Corrupt);
  }
  // Protected DACL: owner rights only, no inherited grants; applied at creation.
  let sddl: Vec<u16> = "D:P(A;;FA;;;OW)".encode_utf16().chain(Some(0)).collect();
  unsafe {
    let mut descriptor = std::ptr::null_mut();
    if ConvertStringSecurityDescriptorToSecurityDescriptorW(
      sddl.as_ptr(),
      1,
      &mut descriptor,
      std::ptr::null_mut(),
    ) == 0
    {
      return Err(io::Error::last_os_error().into());
    }
    let attributes = SECURITY_ATTRIBUTES {
      nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
      lpSecurityDescriptor: descriptor,
      bInheritHandle: 0,
    };
    let handle = CreateFileW(
      path.as_ptr(),
      GENERIC_WRITE,
      0,
      &attributes,
      CREATE_NEW,
      FILE_ATTRIBUTE_NORMAL,
      std::ptr::null_mut(),
    );
    let error = io::Error::last_os_error();
    LocalFree(descriptor);
    if handle == INVALID_HANDLE_VALUE {
      return Err(error.into());
    }
    Ok(fs::File::from_raw_handle(handle))
  }
}
