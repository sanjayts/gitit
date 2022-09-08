use crate::old_raw;
use crate::old_raw::{
    git_commit, git_commit_lookup, git_libgit2_init, git_libgit2_shutdown,
    git_reference_name_to_id, git_repository, git_repository_open,
};
use std::borrow::{BorrowMut, Cow};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::{mem, ptr};

fn main() {
    let repo_path = std::env::args().skip(1).next().expect("Usage: gitit PATH");
    let repo_path = CString::new(repo_path).expect("path contains null chars");
    unsafe {
        check("Initializing libgit", git_libgit2_init());

        let mut repo: *mut git_repository = ptr::null_mut();
        check(
            "Opening Repository",
            git_repository_open(repo.borrow_mut(), repo_path.as_ptr()),
        );

        let ref_name = b"HEAD\0".as_ptr() as *const c_char;
        let oid = {
            let mut oid = mem::MaybeUninit::uninit();
            check(
                "Locating reference",
                git_reference_name_to_id(oid.as_mut_ptr(), repo, ref_name),
            );
            oid.assume_init()
        };

        let mut commit: *mut git_commit = ptr::null_mut();
        check(
            "Looking up git commit",
            git_commit_lookup(commit.borrow_mut(), repo, &oid),
        );
        show_commit(commit);

        old_raw::git_commit_free(commit);
        old_raw::git_repository_free(repo);

        check("Tearing down libgit", git_libgit2_shutdown());
    }
}

fn check(activity: &'static str, status: c_int) -> c_int {
    if status < 0 {
        unsafe {
            let error = &*old_raw::giterr_last();
            println!(
                "Error when doing {} -- {} ({})",
                activity,
                str_from_ptr(error.message),
                status
            );
            std::process::exit(status)
        }
    }
    status
}

unsafe fn show_commit(commit: *const git_commit) {
    let signature = &*old_raw::git_commit_author(commit);

    let email = str_from_ptr(signature.email);
    let name = str_from_ptr(signature.name);
    println!("{} <{}>", name, email);

    let message = str_from_ptr(old_raw::git_commit_message(commit));
    println!("{}", message);
}

unsafe fn str_from_ptr<'a>(ptr: *const c_char) -> Cow<'a, str> {
    CStr::from_ptr(ptr).to_string_lossy()
}
