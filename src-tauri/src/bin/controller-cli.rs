use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;

use uuid::Uuid;

trait ReadWrite: Read + Write {}

impl<T: Read + Write> ReadWrite for T {}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let request_id = Uuid::new_v4().to_string();
    let exit_code = run_cli_with_request_id(
        &args,
        || {
            UnixStream::connect(the_controller_lib::status_socket::socket_path())
                .map(|stream| Box::new(stream) as Box<dyn ReadWrite>)
        },
        &mut stdout,
        &mut stderr,
        &request_id,
    );
    std::process::exit(exit_code);
}

fn run_cli_with_request_id<S, C, Out, Err>(
    args: &[S],
    connect: C,
    stdout: &mut Out,
    stderr: &mut Err,
    request_id: &str,
) -> i32
where
    S: AsRef<str>,
    C: FnOnce() -> std::io::Result<Box<dyn ReadWrite>>,
    Out: Write,
    Err: Write,
{
    let Some((project, key)) = parse_args(args) else {
        let _ = writeln!(
            stderr,
            "Usage: controller-cli env set --project <project> --key <ENV_KEY>"
        );
        return 2;
    };

    let mut stream = match connect() {
        Ok(stream) => stream,
        Err(err)
            if matches!(
                err.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
            let _ = writeln!(stderr, "The Controller app is not running");
            return 1;
        }
        Err(err) => {
            let _ = writeln!(stderr, "Failed to connect to The Controller app: {err}");
            return 1;
        }
    };

    let request = format!("secure-env:set|{project}|{key}|{request_id}\n");
    if let Err(err) = stream.write_all(request.as_bytes()) {
        let _ = writeln!(stderr, "Failed to send request: {err}");
        return 1;
    }

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    if let Err(err) = reader.read_line(&mut response) {
        let _ = writeln!(stderr, "Failed to read response: {err}");
        return 1;
    }

    let response = response.trim_end();
    let mut parts = response.split('|');
    let kind = parts.next().unwrap_or_default();
    let status = parts.next().unwrap_or_default();
    let response_id = parts.next().unwrap_or_default();
    if parts.next().is_some() || kind.is_empty() || status.is_empty() || response_id != request_id {
        let _ = writeln!(stderr, "Invalid response from The Controller app");
        return 1;
    }

    match (kind, status) {
        ("ok", "created" | "updated") => {
            let _ = writeln!(stdout, "{status} {key} for {project}");
            0
        }
        ("error", "cancelled") => {
            let _ = writeln!(stderr, "secure env request cancelled");
            3
        }
        ("error", other) => {
            let _ = writeln!(stderr, "secure env request failed: {other}");
            1
        }
        _ => {
            let _ = writeln!(stderr, "Invalid response from The Controller app");
            1
        }
    }
}

fn parse_args<S: AsRef<str>>(args: &[S]) -> Option<(String, String)> {
    if args.len() != 6
        || args[0].as_ref() != "env"
        || args[1].as_ref() != "set"
        || args[2].as_ref() != "--project"
        || args[4].as_ref() != "--key"
    {
        return None;
    }

    let project = args[3].as_ref().trim();
    let key = args[5].as_ref().trim();
    if project.is_empty() || key.is_empty() {
        return None;
    }

    Some((project.to_string(), key.to_string()))
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read, Write};
    use std::sync::{Arc, Mutex};

    use super::run_cli_with_request_id;

    struct FakeStream {
        reader: Cursor<Vec<u8>>,
        writes: Arc<Mutex<Vec<u8>>>,
    }

    impl FakeStream {
        fn new(response: &str, writes: Arc<Mutex<Vec<u8>>>) -> Self {
            Self {
                reader: Cursor::new(response.as_bytes().to_vec()),
                writes,
            }
        }
    }

    impl Read for FakeStream {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.reader.read(buf)
        }
    }

    impl Write for FakeStream {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.writes.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn sends_secure_env_request_and_prints_redacted_success() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = run_cli_with_request_id(
            &[
                "env",
                "set",
                "--project",
                "demo-project",
                "--key",
                "OPENAI_API_KEY",
            ],
            || {
                Ok(Box::new(FakeStream::new(
                    "ok|updated|req-123\n",
                    writes.clone(),
                )))
            },
            &mut stdout,
            &mut stderr,
            "req-123",
        );

        assert_eq!(exit_code, 0);
        assert_eq!(
            String::from_utf8(writes.lock().unwrap().clone()).unwrap(),
            "secure-env:set|demo-project|OPENAI_API_KEY|req-123\n"
        );
        assert_eq!(
            String::from_utf8(stdout).unwrap(),
            "updated OPENAI_API_KEY for demo-project\n"
        );
        assert!(String::from_utf8(stderr).unwrap().is_empty());
    }

    #[test]
    fn returns_non_zero_when_app_is_not_running() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = run_cli_with_request_id(
            &[
                "env",
                "set",
                "--project",
                "demo-project",
                "--key",
                "OPENAI_API_KEY",
            ],
            || {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "missing socket",
                ))
            },
            &mut stdout,
            &mut stderr,
            "req-123",
        );

        assert_eq!(exit_code, 1);
        assert!(String::from_utf8(stdout).unwrap().is_empty());
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "The Controller app is not running\n"
        );
    }

    #[test]
    fn returns_distinct_exit_code_for_cancelled_requests() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = run_cli_with_request_id(
            &[
                "env",
                "set",
                "--project",
                "demo-project",
                "--key",
                "OPENAI_API_KEY",
            ],
            || {
                Ok(Box::new(FakeStream::new(
                    "error|cancelled|req-123\n",
                    writes.clone(),
                )))
            },
            &mut stdout,
            &mut stderr,
            "req-123",
        );

        assert_eq!(exit_code, 3);
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "secure env request cancelled\n"
        );
    }

    #[test]
    fn rejects_invalid_arguments_with_usage_error() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = run_cli_with_request_id(
            &["env", "set", "--project", "demo-project"],
            || unreachable!("connection should not be attempted"),
            &mut stdout,
            &mut stderr,
            "req-123",
        );

        assert_eq!(exit_code, 2);
        assert!(String::from_utf8(stdout).unwrap().is_empty());
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "Usage: controller-cli env set --project <project> --key <ENV_KEY>\n"
        );
    }
}
