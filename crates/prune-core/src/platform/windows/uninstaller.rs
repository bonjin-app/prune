//! Running the vendor's own uninstaller, the way "Apps & features" does.
//!
//! Separate from [`super::apps`] for the reason `rules.rs` is: none of the deciding needs Windows,
//! and Windows is built in CI but never run by hand here. What is chosen, and what is refused,
//! is plain string logic compiled and tested on every machine. Only starting the process is
//! Windows-only, and it is exercised by the Windows CI job, which runs a real batch file.

/// The command Prune may offer, from the values an application registered.
///
/// `UninstallString` only. Many installers also register `QuietUninstallString`, which is the
/// same uninstall without a window or a confirmation, meant for deployment tools — and the button
/// that runs this says "Run vendor uninstaller" and is one click, with no preview between it and
/// the removal. Preferring the quiet one made that click uninstall the application on the spot,
/// the opposite of what the command is documented to do (the person completes the vendor's own
/// screens) and a way around the rule that nothing is removed before it has been read.
///
/// An application that registered only a quiet command is offered none: there is no version of it
/// that asks.
pub fn interactive_command(get: impl Fn(&str) -> Option<String>) -> Option<String> {
    get("UninstallString")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Why a command line is not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Empty,
    /// A line break or other control character, which no real command line holds.
    ControlCharacter,
    /// A character `cmd.exe` would read as the start of another command or a redirection.
    Metacharacter(char),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Empty => write!(f, "the uninstall command is empty"),
            Refusal::ControlCharacter => {
                write!(f, "the uninstall command contains a control character")
            }
            Refusal::Metacharacter(c) => write!(
                f,
                "the uninstall command contains '{c}' outside quotes, which would start a second command"
            ),
        }
    }
}

/// What to hand `cmd.exe` to start `command` in a window of its own: `/C start "" <command>`.
///
/// The command is the registry's text, kept exactly as it is. It must reach `cmd.exe` verbatim:
/// `Command::arg` re-quotes an argument for the C runtime, turning `"C:\Program Files\App\u.exe"
/// /S` into `"\"C:\Program Files\App\u.exe\" /S"`, which `cmd.exe` does not understand — it has no
/// backslash escape — and the very common uninstaller whose path has a space in it never started.
///
/// `cmd.exe` is a shell, so the text is also checked for what a shell would act on. The registry
/// is not hostile territory, but this is the one place Prune hands a string it did not write to a
/// command interpreter, and `&`, `|`, `<`, `>` or `^` outside quotes would have it run something
/// else after, or instead. `%` is left alone on purpose: `REG_EXPAND_SZ` values hold
/// `%ProgramFiles%` and the shell is what expands it.
pub fn cmd_tail(command: &str) -> Result<String, Refusal> {
    let command = command.trim();
    if command.is_empty() {
        return Err(Refusal::Empty);
    }
    if command.chars().any(|c| c.is_control() && c != '\t') {
        return Err(Refusal::ControlCharacter);
    }
    let mut quoted = false;
    for c in command.chars() {
        match c {
            '"' => quoted = !quoted,
            '&' | '|' | '<' | '>' | '^' if !quoted => return Err(Refusal::Metacharacter(c)),
            _ => {}
        }
    }
    Ok(format!("/C start \"\" {command}"))
}

/// Starts the uninstaller. Windows only.
///
/// The `cmd.exe` that carries out `start` gets no window of its own; the uninstaller it starts
/// shows whatever it shows.
#[cfg(windows)]
pub fn launch(command: &str) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let tail = cmd_tail(command).map_err(|refusal| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, refusal.to_string())
    })?;
    // Not waited for: `start` returns as soon as it has launched the program, and a program it
    // cannot find puts up an error dialog that would hold this call until someone closed it.
    // Standard streams are closed rather than inherited so nothing started here keeps a pipe of
    // ours open after we are done with it.
    std::process::Command::new("cmd")
        .raw_arg(tail)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn registry(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn the_command_that_asks_is_chosen_over_the_one_that_does_not() {
        let get = registry(&[
            ("UninstallString", r#""C:\Program Files\App\unins000.exe""#),
            (
                "QuietUninstallString",
                r#""C:\Program Files\App\unins000.exe" /VERYSILENT"#,
            ),
        ]);
        assert_eq!(
            interactive_command(get).as_deref(),
            Some(r#""C:\Program Files\App\unins000.exe""#)
        );
    }

    #[test]
    fn an_application_with_only_a_quiet_command_is_offered_none() {
        let get = registry(&[("QuietUninstallString", r#""C:\App\u.exe" /S"#)]);
        assert_eq!(interactive_command(get), None);
    }

    #[test]
    fn nothing_registered_and_blank_values_offer_nothing() {
        assert_eq!(interactive_command(registry(&[])), None);
        assert_eq!(
            interactive_command(registry(&[("UninstallString", "  \t ")])),
            None
        );
    }

    #[test]
    fn the_command_is_passed_on_exactly_as_registered() {
        // The case that used to fail: a quoted path with a space in it, then arguments.
        let command = r#""C:\Program Files\Some App\unins000.exe" /SILENT"#;
        assert_eq!(
            cmd_tail(command).unwrap(),
            format!(r#"/C start "" {command}"#)
        );
        // No escaping of the quotes: cmd.exe has no backslash escape.
        assert!(!cmd_tail(command).unwrap().contains(r#"\""#));
    }

    #[test]
    fn an_unquoted_command_is_passed_on_too() {
        assert_eq!(
            cmd_tail("MsiExec.exe /X{0A1B2C3D-0000-0000-0000-000000000000}").unwrap(),
            r#"/C start "" MsiExec.exe /X{0A1B2C3D-0000-0000-0000-000000000000}"#
        );
    }

    #[test]
    fn surrounding_whitespace_is_dropped() {
        assert_eq!(
            cmd_tail("  C:\\a.exe  ").unwrap(),
            r#"/C start "" C:\a.exe"#
        );
    }

    #[test]
    fn a_second_command_cannot_ride_along() {
        for (cmd, bad) in [
            (r"C:\a.exe & calc", '&'),
            (r"C:\a.exe | more", '|'),
            (r"C:\a.exe > C:\out.txt", '>'),
            (r"C:\a.exe < C:\in.txt", '<'),
            (r"C:\a.exe ^& calc", '^'),
            // The quote closes before the ampersand, so the shell sees it.
            (r#""C:\a.exe" /S & calc"#, '&'),
        ] {
            assert_eq!(cmd_tail(cmd), Err(Refusal::Metacharacter(bad)), "{cmd}");
        }
    }

    #[test]
    fn the_same_characters_inside_quotes_are_just_characters() {
        // A folder called "R&D" is a folder.
        let command = r#""C:\R&D\Tools (x86)\u.exe" --name "a|b""#;
        assert_eq!(
            cmd_tail(command).unwrap(),
            format!(r#"/C start "" {command}"#)
        );
    }

    #[test]
    fn environment_references_are_left_for_the_shell_to_expand() {
        assert!(cmd_tail(r"%ProgramFiles%\App\uninstall.exe /S").is_ok());
    }

    #[test]
    fn line_breaks_and_empty_commands_are_refused() {
        assert_eq!(cmd_tail(""), Err(Refusal::Empty));
        assert_eq!(cmd_tail("   "), Err(Refusal::Empty));
        assert_eq!(
            cmd_tail("C:\\a.exe\r\ncalc"),
            Err(Refusal::ControlCharacter)
        );
        assert_eq!(cmd_tail("C:\\a.exe\0"), Err(Refusal::ControlCharacter));
    }

    #[test]
    fn a_refusal_says_why() {
        let text = cmd_tail("a & b").unwrap_err().to_string();
        assert!(text.contains('&') && text.contains("second command"));
    }
}

/// Starts a real batch file through [`launch`], which is the only way to know that a path with a
/// space in it and quoted arguments reach it intact. Windows only, so it runs in the Windows CI job.
#[cfg(all(test, windows))]
mod launch_tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn a_quoted_path_with_a_space_and_quoted_arguments_arrive_intact() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("A Folder With Spaces");
        std::fs::create_dir_all(&home).unwrap();
        let marker = home.join("what it was given.txt");
        let script = home.join("uninstall me.cmd");
        // Writes the arguments it received into the marker.
        std::fs::write(
            &script,
            format!("@echo off\r\n>\"{}\" echo %*\r\n", marker.display()),
        )
        .unwrap();

        launch(&format!("\"{}\" /S \"two words\"", script.display())).unwrap();

        let deadline = Instant::now() + Duration::from_secs(20);
        while !marker.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(marker.exists(), "the uninstaller never ran");
        // Give the batch file a moment to finish writing before reading.
        std::thread::sleep(Duration::from_millis(300));
        let got = std::fs::read_to_string(&marker).unwrap();
        assert!(got.contains("/S"), "arguments lost: {got:?}");
        assert!(got.contains("\"two words\""), "quoting lost: {got:?}");
    }

    #[test]
    fn a_command_with_a_second_command_in_it_is_not_run() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("should not exist.txt");
        let err = launch(&format!(
            "cmd /C exit 0 & echo x > \"{}\"",
            marker.display()
        ))
        .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        std::thread::sleep(Duration::from_millis(500));
        assert!(!marker.exists());
    }
}
