use crate::git::Repository;
use std::ffi::CString;

mod git;
mod old_main;
mod old_raw;

const NONE: &str = "(none)";

fn main() {
    let repo_path = std::env::args().skip(1).next().expect("Usage: gitit PATH");
    let repo = Repository::open(repo_path).expect("Failed to open repository");
    let oid = repo
        .reference_name_to_oid("HEAD")
        .expect("Failed to get OID");
    let commit = repo.find_commit(&oid).expect("Failed to get commit");
    let author = commit.author();
    println!(
        "{} <{}>",
        author.name().unwrap_or(NONE),
        author.email().unwrap_or(NONE)
    );
    println!("{}", commit.message().unwrap_or(NONE));
}
