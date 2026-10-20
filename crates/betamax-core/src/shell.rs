//! Shell launch normalization and terminal environment defaults.
//!
//! VHS examples commonly assume a clean shell with a simple colored `>` prompt. This module applies
//! that convention without requiring every tape to configure a prompt manually.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use miette::miette;
use portable_pty::CommandBuilder;

use crate::Result;

/// Shell used when a tape has no `Set Shell`.
///
/// This matches VHS, so tape commands behave the same regardless of the host user's `$SHELL`.
pub(crate) const DEFAULT_SHELL: &str = "bash";

/// Shell family that Betamax knows how to make deterministic for examples.
///
/// The distinction matters because each shell has a different mechanism for disabling startup
/// files and installing a VHS-style prompt. Unknown shells still receive generic terminal color
/// defaults, but Betamax avoids guessing shell-specific prompt syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellKind {
    /// GNU/bash-compatible startup flags and PS1 handling.
    Bash,
    /// Fish startup flags and prompt function handling.
    Fish,
    /// Unknown shell; only generic terminal environment variables are applied.
    Other,
    /// Zsh startup flags and prompt handling.
    Zsh,
}

/// Normalized shell argv plus inferred shell family.
///
/// `Settings` stores the raw tape-provided shell argv. This type is created immediately before
/// spawning the PTY so prompt/environment setup can use the same shell classification as argv
/// normalization.
pub(crate) struct ShellLaunch {
    /// Final argv passed to the PTY command builder.
    pub(crate) argv: Vec<OsString>,
    /// Shell family inferred from `argv[0]`.
    pub(crate) kind: ShellKind,
}

impl ShellLaunch {
    /// Normalize a user-provided shell argv.
    ///
    /// If the tape provided only a shell binary, Betamax adds flags that suppress user startup
    /// files and history where practical. If the tape provided explicit arguments, they are
    /// preserved exactly because the user has taken control of shell startup behavior.
    pub(crate) fn from_argv(argv: &[OsString]) -> Self {
        let mut argv = argv.to_vec();
        if argv.is_empty() {
            argv.push(OsString::from(DEFAULT_SHELL));
        }

        let kind = shell_kind(&argv[0]);
        if argv.len() != 1 {
            return Self { argv, kind };
        }

        match kind {
            ShellKind::Bash => {
                argv.extend(
                    ["--noprofile", "--norc", "--login", "+o", "history"].map(OsString::from),
                );
            }
            ShellKind::Fish => {
                argv.extend(
                    [
                        "--login",
                        "--no-config",
                        "--private",
                        "-C",
                        "function fish_greeting; end",
                        "-C",
                        "function fish_prompt; set_color 5B56E0; echo -n \"> \"; set_color normal; end",
                    ]
                    .map(OsString::from),
                );
            }
            ShellKind::Zsh => {
                argv.extend(["--histnostore", "--no-rcs"].map(OsString::from));
            }
            ShellKind::Other => {}
        }

        Self { argv, kind }
    }
}

/// Fail before PTY startup when the shell program can't be found.
///
/// A program containing a path separator must exist as given. A bare name is looked up on `PATH`,
/// using a tape `Env PATH` override when present because that is the `PATH` the spawn sees.
/// Checking here gives a targeted error instead of a vague PTY spawn failure, which matters most
/// for the bash default on hosts without bash.
pub(crate) fn ensure_shell_exists(program: &OsStr, env: &[(String, String)]) -> Result<()> {
    let found = if Path::new(program).components().count() > 1 {
        Path::new(program).is_file()
    } else {
        let path = env
            .iter()
            .rev()
            .find(|(key, _)| key == "PATH")
            .map(|(_, value)| OsString::from(value))
            .or_else(|| std::env::var_os("PATH"))
            .unwrap_or_default();
        std::env::split_paths(&path).any(|dir| dir.join(program).is_file())
    };
    if found {
        return Ok(());
    }
    let name = program.to_string_lossy();
    if program == DEFAULT_SHELL {
        return Err(miette!(
            "shell `{name}` not found on PATH; tapes run in bash unless they set a shell, \
             so install bash or add `Set Shell \"sh\"` (or another installed shell) to the tape"
        )
        .into());
    }
    Err(miette!("shell `{name}` not found; check the tape's `Set Shell` value").into())
}

/// Infer a supported shell family from the executable name.
fn shell_kind(shell: &OsString) -> ShellKind {
    let name = Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    match name {
        "bash" => ShellKind::Bash,
        "fish" => ShellKind::Fish,
        "zsh" => ShellKind::Zsh,
        _ => ShellKind::Other,
    }
}

/// Apply terminal defaults and tape environment variables to the spawned command.
///
/// Betamax defaults to truecolor terminal behavior and removes `NO_COLOR` so examples render with
/// color by default. Tape-provided `Env` entries are applied last, so a tape can deliberately opt
/// back into `NO_COLOR`, override `TERM`, or replace the prompt environment.
pub(crate) fn apply_terminal_environment(
    command: &mut CommandBuilder,
    shell: ShellKind,
    env: &[(String, String)],
) {
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    command.env_remove("NO_COLOR");
    command.env("BASH_SILENCE_DEPRECATION_WARNING", "1");

    match shell {
        ShellKind::Bash | ShellKind::Other => {
            command.env("PS1", r"\[\e[38;2;90;86;224m\]> \[\e[0m\]");
        }
        ShellKind::Fish => {}
        ShellKind::Zsh => {
            command.env("PROMPT", "%F{#5B56E0}> %f");
            command.env("RPROMPT", "");
            command.env("RPS1", "");
        }
    }

    for (key, value) in env {
        command.env(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_argv_defaults_to_clean_bash() {
        let launch = ShellLaunch::from_argv(&[]);

        assert_eq!(launch.kind, ShellKind::Bash);
        assert_eq!(
            launch.argv,
            ["bash", "--noprofile", "--norc", "--login", "+o", "history"].map(OsString::from)
        );
    }

    #[test]
    fn missing_default_shell_suggests_set_shell() {
        let env = [("PATH".to_string(), "/nonexistent-betamax-dir".to_string())];

        let error = ensure_shell_exists(OsStr::new("bash"), &env).unwrap_err();

        let message = error.to_string();
        assert!(
            message.contains("shell `bash` not found on PATH"),
            "{message}"
        );
        assert!(message.contains("Set Shell"), "{message}");
    }

    #[test]
    fn missing_shell_path_is_an_error() {
        let error = ensure_shell_exists(OsStr::new("/nonexistent/zsh"), &[]).unwrap_err();

        assert!(error
            .to_string()
            .contains("shell `/nonexistent/zsh` not found"));
    }

    #[test]
    fn shell_lookup_uses_tape_path() {
        let dir = std::env::temp_dir();
        let env = [("PATH".to_string(), "/nonexistent-betamax-dir".to_string())];
        assert!(ensure_shell_exists(OsStr::new("sh"), &env).is_err());

        let env = [(
            "PATH".to_string(),
            format!("/nonexistent-betamax-dir:/bin:{}", dir.display()),
        )];
        ensure_shell_exists(OsStr::new("sh"), &env).unwrap();
    }

    #[test]
    fn terminal_environment_defaults_to_color() {
        let mut command = CommandBuilder::new("sh");
        command.env("NO_COLOR", "1");

        apply_terminal_environment(&mut command, ShellKind::Bash, &[]);

        assert_eq!(
            command.get_env("TERM").and_then(|value| value.to_str()),
            Some("xterm-256color")
        );
        assert_eq!(
            command
                .get_env("COLORTERM")
                .and_then(|value| value.to_str()),
            Some("truecolor")
        );
        assert!(command.get_env("NO_COLOR").is_none());
        assert_eq!(
            command.get_env("PS1").and_then(|value| value.to_str()),
            Some(r"\[\e[38;2;90;86;224m\]> \[\e[0m\]")
        );
        assert_eq!(
            command
                .get_env("BASH_SILENCE_DEPRECATION_WARNING")
                .and_then(|value| value.to_str()),
            Some("1")
        );
    }

    #[test]
    fn tape_environment_overrides_color_defaults() {
        let mut command = CommandBuilder::new("sh");
        let env = vec![
            ("TERM".to_string(), "dumb".to_string()),
            ("NO_COLOR".to_string(), "1".to_string()),
        ];

        apply_terminal_environment(&mut command, ShellKind::Bash, &env);

        assert_eq!(
            command.get_env("TERM").and_then(|value| value.to_str()),
            Some("dumb")
        );
        assert_eq!(
            command.get_env("NO_COLOR").and_then(|value| value.to_str()),
            Some("1")
        );
    }

    #[test]
    fn expands_known_shells_to_vhs_style_launches() {
        let launch = ShellLaunch::from_argv(&[OsString::from("bash")]);

        assert_eq!(launch.kind, ShellKind::Bash);
        assert_eq!(
            launch.argv,
            ["bash", "--noprofile", "--norc", "--login", "+o", "history"].map(OsString::from)
        );

        let launch = ShellLaunch::from_argv(&[OsString::from("/bin/zsh")]);

        assert_eq!(launch.kind, ShellKind::Zsh);
        assert_eq!(
            launch.argv,
            ["/bin/zsh", "--histnostore", "--no-rcs"].map(OsString::from)
        );
    }

    #[test]
    fn leaves_explicit_shell_arguments_untouched() {
        let launch = ShellLaunch::from_argv(&[
            OsString::from("bash"),
            OsString::from("--noprofile"),
            OsString::from("-i"),
        ]);

        assert_eq!(launch.kind, ShellKind::Bash);
        assert_eq!(
            launch.argv,
            ["bash", "--noprofile", "-i"].map(OsString::from)
        );
    }
}
