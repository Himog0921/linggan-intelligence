//! Record the actual local build identity, never a hand-written source baseline.
use std::{path::PathBuf, process::Command};
fn git(args: &[&str]) -> Option<String> {
    let result = Command::new("git").args(args).output().ok()?;
    result.status.success().then(||String::from_utf8_lossy(&result.stdout).trim().to_owned())
}
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src");
    for name in ["HEAD", "index"] {
        if let Some(path) = git(&["rev-parse","--git-path",name]) {
            println!("cargo:rerun-if-changed={}", PathBuf::from(path).display());
        }
    }
    if let Some(reference) = git(&["symbolic-ref","-q","HEAD"])
        && let Some(path) = git(&["rev-parse","--git-path",&reference])
    { println!("cargo:rerun-if-changed={path}"); }
    if let Some(revision) = git(&["rev-parse","HEAD"])
        && revision.len()==40 && revision.bytes().all(|b|b.is_ascii_hexdigit())
    {
        println!("cargo:rustc-env=COMMENT_STUDY_ENGINE_REVISION={revision}");
        let dirty = git(&["status","--porcelain","--untracked-files=normal"]).is_none_or(|s|!s.is_empty());
        println!("cargo:rustc-env=COMMENT_STUDY_ENGINE_DIRTY={dirty}");
    }
}
