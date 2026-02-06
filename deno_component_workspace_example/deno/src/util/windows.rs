// Copyright 2018-2026 the Deno authors. MIT license.

/// Ensures that stdin, stdout, and stderr are open and have valid HANDLEs
/// associated with them. There are many places where a `std::fs::File` is
/// constructed from a stdio handle; if the handle is null this causes a panic.
pub fn ensure_stdio_open() {
  #[cfg(windows)]
  // SAFETY: winapi calls
  unsafe {
    use std::mem::size_of;

    use winapi::{
      shared::{
        minwindef::{DWORD, FALSE, TRUE},
        ntdef::NULL,
        winerror::ERROR_INVALID_HANDLE,
      },
      um::{
        errhandlingapi::GetLastError,
        fileapi::{CreateFileA, OPEN_EXISTING},
        handleapi::{GetHandleInformation, INVALID_HANDLE_VALUE},
        minwinbase::SECURITY_ATTRIBUTES,
        processenv::{GetStdHandle, SetStdHandle},
        winbase::{STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE},
        winnt::{
          FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_READ_ATTRIBUTES,
          FILE_SHARE_READ, FILE_SHARE_WRITE,
        },
      },
    };

    for std_handle in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
      // Check whether stdio handle is open.
      let is_valid = match GetStdHandle(std_handle) {
        NULL | INVALID_HANDLE_VALUE => false,
        handle => {
          // The stdio handle is open; check whether its handle is valid.
          let mut flags: DWORD = 0;
          match GetHandleInformation(handle, &mut flags) {
            TRUE => true,
            FALSE if GetLastError() == ERROR_INVALID_HANDLE => false,
            FALSE => {
              panic!("GetHandleInformation failed (error {})", GetLastError());
            }
            _ => unreachable!(),
          }
        }
      };

      if !is_valid {
        // Open NUL device.
        let desired_access = match std_handle {
          STD_INPUT_HANDLE => FILE_GENERIC_READ,
          _ => FILE_GENERIC_WRITE | FILE_READ_ATTRIBUTES,
        };
        let security_attributes = SECURITY_ATTRIBUTES {
          nLength: size_of::<SECURITY_ATTRIBUTES>() as DWORD,
          lpSecurityDescriptor: NULL,
          bInheritHandle: TRUE,
        };
        let file_handle = CreateFileA(
          b"\\\\?\\NUL\0" as *const _ as *mut _,
          desired_access,
          FILE_SHARE_READ | FILE_SHARE_WRITE,
          &security_attributes as *const _ as *mut _,
          OPEN_EXISTING,
          FILE_ATTRIBUTE_NORMAL,
          NULL,
        );
        match file_handle {
          NULL => unreachable!(),
          INVALID_HANDLE_VALUE => {
            panic!("Could not open NUL device (error {})", GetLastError());
          }
          _ => {}
        }

        // Assign the opened NUL handle to the missing stdio handle.
        let success = SetStdHandle(std_handle, file_handle);
        match success {
          TRUE => {}
          FALSE => panic!("SetStdHandle failed (error {})", GetLastError()),
          _ => unreachable!(),
        }
      }
    }
  }
}
