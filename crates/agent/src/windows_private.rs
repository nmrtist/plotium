//! Use the process token SID, independent of account names or shell tools.
use std::{
    io,
    os::windows::{ffi::OsStrExt, fs::MetadataExt},
    path::Path,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree},
    Security::{Authorization::*, *},
    Storage::FileSystem::{FILE_ALL_ACCESS, FILE_ATTRIBUTE_REPARSE_POINT},
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};
struct LocalAllocation(*mut core::ffi::c_void);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        // SAFETY: GetNamedSecurityInfoW/SetEntriesInAclW return LocalAlloc memory.
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn user() -> io::Result<Vec<usize>> {
    let mut token = null_mut();
    // SAFETY: All output pointers refer to initialized local storage. The process
    // pseudo-handle is valid, and the real token handle is closed below.
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut size = 0;
        GetTokenInformation(token, TokenUser, null_mut(), 0, &mut size);
        let mut storage = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        let success = GetTokenInformation(
            token,
            TokenUser,
            storage.as_mut_ptr().cast(),
            size,
            &mut size,
        );
        let error = io::Error::last_os_error();
        CloseHandle(token);
        if success == 0 {
            return Err(error);
        }
        Ok(storage)
    }
}
fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
fn failure() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "MCP discovery must be private to the current Windows user and must not be a reparse point",
    )
}
fn no_reparse(path: &Path) -> io::Result<()> {
    if std::fs::symlink_metadata(path)?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(failure());
    }
    Ok(())
}
pub(super) fn restrict(dir: &Path) -> io::Result<()> {
    no_reparse(dir)?;
    let storage = user()?;
    let name = wide(dir);
    // SAFETY: TOKEN_USER and its SID live in aligned storage through both calls.
    // The ACL is released only after SetNamedSecurityInfoW has copied it.
    unsafe {
        let sid = (*(storage.as_ptr().cast::<TOKEN_USER>())).User.Sid;
        let access = EXPLICIT_ACCESS_W {
            grfAccessPermissions: FILE_ALL_ACCESS,
            grfAccessMode: SET_ACCESS,
            grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: null_mut(),
                MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_USER,
                ptstrName: sid.cast(),
            },
        };
        let mut acl = null_mut();
        let result = SetEntriesInAclW(1, &access, null(), &mut acl);
        if result != 0 {
            return Err(io::Error::from_raw_os_error(result as i32));
        }
        let _allocation = LocalAllocation(acl.cast());
        let result = SetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            acl,
            null(),
        );
        if result != 0 {
            return Err(io::Error::from_raw_os_error(result as i32));
        }
    }
    validate(dir)
}
pub(super) fn validate(path: &Path) -> io::Result<()> {
    no_reparse(path)?;
    let storage = user()?;
    let name = wide(path);
    // SAFETY: The OS validates the descriptor; owner and ACL borrow its allocation.
    // ACE pointers are accessed only after GetAce succeeds, and the descriptor
    // and process token SID storage stay alive until validation is complete.
    unsafe {
        let sid = (*(storage.as_ptr().cast::<TOKEN_USER>())).User.Sid;
        let mut owner = null_mut();
        let mut acl = null_mut();
        let mut descriptor = null_mut();
        let result = GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut acl,
            null_mut(),
            &mut descriptor,
        );
        if result != 0 {
            return Err(io::Error::from_raw_os_error(result as i32));
        }
        let _allocation = LocalAllocation(descriptor);
        if owner.is_null() || EqualSid(owner, sid) == 0 || acl.is_null() {
            return Err(failure());
        }
        for index in 0..(*acl).AceCount {
            let mut ace = null_mut();
            if GetAce(acl, u32::from(index), &mut ace) == 0 {
                return Err(io::Error::last_os_error());
            }
            // Only simple allow entries for this user are accepted. Reject
            // object/callback ACEs rather than interpreting variable layouts.
            let header = &*ace.cast::<ACE_HEADER>();
            if header.AceType != 0 {
                return Err(failure());
            }
            let allowed = &*ace.cast::<ACCESS_ALLOWED_ACE>();
            let granted_sid = std::ptr::addr_of!(allowed.SidStart).cast_mut().cast();
            if EqualSid(granted_sid, sid) == 0 {
                return Err(failure());
            }
        }
    }
    Ok(())
}
