//! Where to find `git.exe` on Windows: on `PATH`, or where Git for Windows installs itself for
//! everyone or for the current user.

use std::path::PathBuf;

pub fn locate() -> Option<PathBuf> {
    super::super::find_on_path("git").or_else(|| {
        [
            std::env::var_os("ProgramFiles").map(|d| PathBuf::from(d).join(r"Git\cmd\git.exe")),
            std::env::var_os("LOCALAPPDATA")
                .map(|d| PathBuf::from(d).join(r"Programs\Git\cmd\git.exe")),
        ]
        .into_iter()
        .flatten()
        .find(|git| git.is_file())
    })
}
