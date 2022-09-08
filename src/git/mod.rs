use libc::{c_char, c_int};
use std::borrow::{Borrow, BorrowMut};
use std::ffi::{CStr, CString, NulError};
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use std::path::Path;
use std::sync::Once;
use std::{error, mem, ptr};

mod raw;

#[derive(Debug)]
pub struct GitError {
    code: i32,
    klass: i32,
    message: String,
}

/// Wide hash of the contents of a git object (commit, tree, blob etc)
pub struct Oid {
    raw: raw::git_oid,
}

impl Display for GitError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}

impl error::Error for GitError {}

pub type GitResult<T> = Result<T, GitError>;

fn check(code: c_int) -> GitResult<c_int> {
    if code >= 0 {
        return Ok(code);
    }
    unsafe {
        let error = raw::giterr_last();
        let message = CStr::from_ptr((*error).message)
            .to_string_lossy()
            .into_owned();
        Err(GitError {
            message,
            code,
            klass: (*error).klass as i32,
        })
    }
}

pub struct Repository {
    raw: *mut raw::git_repository,
}

pub struct Commit<'repo> {
    raw: *mut raw::git_commit,
    _marker: PhantomData<&'repo Repository>,
}

impl<'repo> Commit<'repo> {
    pub fn author(&self) -> Signature {
        unsafe {
            Signature {
                raw: raw::git_commit_author(self.raw),
                _marker: PhantomData,
            }
        }
    }

    pub fn message(&self) -> Option<&str> {
        unsafe { char_ptr_to_string(self, raw::git_commit_message(self.raw)) }
    }
}

unsafe fn char_ptr_to_string<T>(_owner: &T, ptr: *const c_char) -> Option<&str> {
    if ptr.is_null() {
        None
    } else {
        CStr::from_ptr(ptr).to_str().ok()
    }
}

pub struct Signature<'text> {
    raw: *const raw::git_signature,
    _marker: PhantomData<&'text str>,
}

impl<'text> Signature<'text> {
    pub fn name(&self) -> Option<&str> {
        unsafe { char_ptr_to_string(self, (*self.raw).name) }
    }

    pub fn email(&self) -> Option<&str> {
        unsafe { char_ptr_to_string(self, (*self.raw).email) }
    }
}

impl<'repo> Drop for Commit<'repo> {
    fn drop(&mut self) {
        unsafe { raw::git_commit_free(self.raw) }
    }
}

static INIT: Once = Once::new();

impl Repository {
    pub fn open<P: AsRef<Path>>(mut path: P) -> GitResult<Self> {
        ensure_initialized();
        unsafe {
            let path = path_to_cstring(path.as_ref())?;
            let mut repo: *mut raw::git_repository = std::ptr::null_mut();
            raw::git_repository_open(&mut repo, path.as_ptr());
            Ok(Repository { raw: repo })
        }
    }

    pub fn reference_name_to_oid(&self, ref_name: &str) -> GitResult<Oid> {
        let ref_name = CString::new(ref_name)?;
        let oid = unsafe {
            let mut oid = mem::MaybeUninit::uninit();
            check(raw::git_reference_name_to_id(
                oid.as_mut_ptr(),
                self.raw,
                ref_name.as_ptr() as *const c_char,
            ))?;
            oid.assume_init()
        };
        Ok(Oid { raw: oid })
    }

    pub fn find_commit(&self, oid: &Oid) -> GitResult<Commit> {
        let mut commit: *mut raw::git_commit = ptr::null_mut();
        unsafe {
            check(raw::git_commit_lookup(
                commit.borrow_mut(),
                self.raw,
                oid.raw.borrow(),
            ))?;
            Ok(Commit {
                raw: commit,
                _marker: PhantomData,
            })
        }
    }
}

#[cfg(unix)]
fn path_to_cstring(path: &Path) -> GitResult<CString> {
    use std::os::unix::ffi::OsStrExt;
    Ok(CString::new(path.as_os_str().as_bytes())?)
}

#[cfg(windows)]
fn path_to_cstring(path: &Path) -> GitResult<CString> {
    match path.to_str() {
        Some(s) => Ok(CString::new(s)?),
        None => {
            let message = format!(
                "Can't open repo since not a valid utf-8 path -- {}",
                path.display()
            );
            Err(message.into())
        }
    }
}

impl From<String> for GitError {
    fn from(message: String) -> Self {
        GitError {
            code: -1,
            klass: 0,
            message,
        }
    }
}

impl From<NulError> for GitError {
    fn from(e: NulError) -> Self {
        GitError {
            code: -1,
            klass: 0,
            message: e.to_string(),
        }
    }
}

impl Drop for Repository {
    fn drop(&mut self) {
        unsafe { raw::git_repository_free(self.raw) }
    }
}

fn ensure_initialized() {
    INIT.call_once(|| unsafe {
        check(raw::git_libgit2_init()).expect("Init of git failed");
        assert_eq!(libc::atexit(on_shutdown), 0);
    });
}

extern "C" fn on_shutdown() {
    unsafe {
        // Since atexit is a libc exit handler, we can't panic since panic is not supposed to
        // cross language boundaries. So we simply print the error on STDERR and abort processing
        if let Err(e) = check(raw::git_libgit2_shutdown()) {
            eprintln!("Failed to tear down libgit -- {}", e);
            std::process::abort()
        }
    }
}
