use std::collections::HashMap;
use std::io::Read;
use std::process::{Command, Stdio};

/// Variables that are shell-internal and should not be propagated
/// to the current process.
const SKIP_VARS: &[&str] = &["_", "SHLVL", "OLDPWD", "PWD"];

/// Maximum time to wait for the shell to produce env output.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// Marker printed before `env` output so we can skip `.zshrc` noise
/// (motd, greeting, fortune, etc.) that precedes the actual environment.
const ENV_MARKER: &str = "___THE_CONTROLLER_ENV___";

/// Resolve the user's shell environment and apply it to the current process.
///
/// macOS GUI apps inherit the minimal launchd environment which does not
/// include variables set in `.zshrc`, `.zprofile`, etc. This function
/// spawns the user's `$SHELL` as an interactive login shell (`-ilc`) and
/// captures the resulting environment so child processes (PTY sessions,
/// broker) see the same variables the user would in a terminal.
///
/// Must be called early in startup, before spawning any threads, because
/// `std::env::set_var` is not thread-safe.
pub fn inherit_shell_env() {
    tracing::info!("starting shell environment inheritance");
    let env = resolve_shell_env();
    let count = env.len();
    for (key, val) in env {
        std::env::set_var(&key, &val);
    }
    tracing::info!(count, "shell environment applied");
}

fn resolve_shell_env() -> HashMap<String, String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| {
        tracing::warn!("$SHELL not set, falling back to /bin/zsh");
        "/bin/zsh".to_string()
    });
    tracing::debug!(shell = %shell, "detected user shell");

    let script = format!("echo '{}'; /usr/bin/env", ENV_MARKER);

    let mut child = match Command::new(&shell)
        .args(["-ilc", &script])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(shell = %shell, error = %e, "failed to spawn shell for env inheritance");
            return HashMap::new();
        }
    };

    // Wait with timeout to avoid blocking app startup if the shell hangs.
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > TIMEOUT {
                    tracing::error!(shell = %shell, "shell env capture timed out, killing child process");
                    let _ = child.kill();
                    let _ = child.wait();
                    return HashMap::new();
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                tracing::error!(error = %e, "error waiting for shell process");
                return HashMap::new();
            }
        }
    }

    let mut stdout = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_end(&mut stdout);
    }

    parse_env_output(&stdout)
}

/// Parse newline-separated `env` output into a HashMap.
///
/// Lines before `ENV_MARKER` are skipped (shell startup noise).
/// Handles multi-line values by checking whether each line starts a new
/// valid `KEY=VALUE` pair. Lines that don't match a valid env var name
/// before `=` are treated as continuations of the previous value.
fn parse_env_output(data: &[u8]) -> HashMap<String, String> {
    let text = String::from_utf8_lossy(data);
    let mut env = HashMap::new();
    let mut current_key = String::new();
    let mut current_val = String::new();
    let mut past_marker = false;

    for line in text.lines() {
        if !past_marker {
            if line.trim() == ENV_MARKER {
                past_marker = true;
            }
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            if is_valid_env_name(key) && !SKIP_VARS.contains(&key) {
                if !current_key.is_empty() {
                    env.insert(
                        std::mem::take(&mut current_key),
                        std::mem::take(&mut current_val),
                    );
                }
                current_key = key.to_string();
                current_val = val.to_string();
                continue;
            }
        }
        // Continuation line (part of a multi-line value)
        if !current_key.is_empty() {
            current_val.push('\n');
            current_val.push_str(line);
        }
    }
    if !current_key.is_empty() {
        env.insert(current_key, current_val);
    }

    env
}

fn is_valid_env_name(s: &str) -> bool {
    !s.is_empty()
        && s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_marker(env_lines: &str) -> Vec<u8> {
        format!("{}\n{}", ENV_MARKER, env_lines).into_bytes()
    }

    #[test]
    fn parses_simple_env_output() {
        let data = with_marker("HOME=/Users/test\nPATH=/usr/bin:/bin\nMY_VAR=hello\n");
        let env = parse_env_output(&data);
        assert_eq!(env.get("HOME").unwrap(), "/Users/test");
        assert_eq!(env.get("PATH").unwrap(), "/usr/bin:/bin");
        assert_eq!(env.get("MY_VAR").unwrap(), "hello");
    }

    #[test]
    fn skips_noise_before_marker() {
        let data = format!(
            "Welcome to zsh!\nFAKE=should_be_skipped\n{}\nREAL=yes\n",
            ENV_MARKER
        );
        let env = parse_env_output(data.as_bytes());
        assert!(!env.contains_key("FAKE"));
        assert_eq!(env.get("REAL").unwrap(), "yes");
    }

    #[test]
    fn skips_internal_vars() {
        let data = with_marker("_=/usr/bin/env\nSHLVL=2\nPWD=/tmp\nOLDPWD=/home\nKEEP=yes\n");
        let env = parse_env_output(&data);
        assert!(!env.contains_key("_"));
        assert!(!env.contains_key("SHLVL"));
        assert!(!env.contains_key("PWD"));
        assert!(!env.contains_key("OLDPWD"));
        assert_eq!(env.get("KEEP").unwrap(), "yes");
    }

    #[test]
    fn handles_multiline_values() {
        let data = with_marker("NORMAL=abc\nMULTI=line1\nline2\nline3\nAFTER=ok\n");
        let env = parse_env_output(&data);
        assert_eq!(env.get("NORMAL").unwrap(), "abc");
        assert_eq!(env.get("MULTI").unwrap(), "line1\nline2\nline3");
        assert_eq!(env.get("AFTER").unwrap(), "ok");
    }

    #[test]
    fn handles_values_with_equals() {
        let data = with_marker("MY_VAR=key=value\n");
        let env = parse_env_output(&data);
        assert_eq!(env.get("MY_VAR").unwrap(), "key=value");
    }

    #[test]
    fn handles_empty_values() {
        let data = with_marker("EMPTY=\nNONEMPTY=x\n");
        let env = parse_env_output(&data);
        assert_eq!(env.get("EMPTY").unwrap(), "");
        assert_eq!(env.get("NONEMPTY").unwrap(), "x");
    }

    #[test]
    fn handles_empty_input() {
        let env = parse_env_output(b"");
        assert!(env.is_empty());
    }

    #[test]
    fn returns_empty_without_marker() {
        let data = b"HOME=/Users/test\nPATH=/usr/bin\n";
        let env = parse_env_output(data);
        assert!(env.is_empty());
    }

    #[test]
    fn valid_env_names() {
        assert!(is_valid_env_name("HOME"));
        assert!(is_valid_env_name("MY_VAR"));
        assert!(is_valid_env_name("_PRIVATE"));
        assert!(is_valid_env_name("VAR123"));
        assert!(!is_valid_env_name(""));
        assert!(!is_valid_env_name("123ABC"));
        assert!(!is_valid_env_name("my-var"));
        assert!(!is_valid_env_name("has space"));
    }
}
