//! Real CLI processes, with private state directories and explicit pair confirmation.
use std::{
    fs,
    io::Write,
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Receiver {
    process: Child,
    directory: tempfile::TempDir,
    address: String,
    log: PathBuf,
}
impl Receiver {
    fn start(reject: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let received = dir
            .path()
            .join("received")
            .display()
            .to_string()
            .replace('\\', "\\\\");
        fs::write(dir.path().join("config.toml"),format!("port = {port}\nreceive_directory = \"{received}\"\nproviders = [\"manual\"]\ntimeout_seconds = 30\n")).unwrap();
        let log = dir.path().join("stderr.log");
        let flag = if reject {
            "--auto-reject"
        } else {
            "--auto-accept"
        };
        let mut process = Command::new(env!("CARGO_BIN_EXE_meshdrop"))
            .arg("--state-dir")
            .arg(dir.path())
            .args(["receive", flag])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(fs::File::create(&log).unwrap())
            .spawn()
            .unwrap();
        process
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"y\ny\ny\n")
            .unwrap();
        let result = Self {
            process,
            directory: dir,
            address: format!("127.0.0.1:{port}"),
            log,
        };
        wait_listening(&result);
        result
    }
    fn pair(&self, other: &Receiver) {
        let mut process = Command::new(env!("CARGO_BIN_EXE_meshdrop"))
            .arg("--state-dir")
            .arg(self.directory.path())
            .args(["pair", &other.address, "--advertise", &self.address])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        process.stdin.take().unwrap().write_all(b"y\n").unwrap();
        assert!(process.wait().unwrap().success());
    }
    fn send(&self, other: &Receiver, file: &PathBuf) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_meshdrop"))
            .arg("--state-dir")
            .arg(self.directory.path())
            .args(["send", &other.address])
            .arg(file)
            .output()
            .unwrap()
    }
}
fn wait_listening(receiver: &Receiver) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !fs::read_to_string(&receiver.log)
        .unwrap_or_default()
        .contains("listening")
    {
        assert!(
            Instant::now() < deadline,
            "receiver startup timed out: {}",
            fs::read_to_string(&receiver.log).unwrap_or_default()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        if self.process.try_wait().expect("inspect receiver").is_none() {
            self.process.kill().expect("kill receiver");
            self.process.wait().expect("reap receiver");
        }
    }
}
#[test]
fn two_receivers_pair_and_transfer_in_both_directions() {
    let first = Receiver::start(false);
    let second = Receiver::start(false);
    first.pair(&second);
    let one = first.directory.path().join("one.txt");
    fs::write(&one, b"from first").unwrap();
    let two = second.directory.path().join("two.txt");
    fs::write(&two, b"from second").unwrap();
    assert!(first.send(&second, &one).status.success());
    assert_eq!(
        fs::read(second.directory.path().join("received/one.txt")).unwrap(),
        b"from first"
    );
    assert!(second.send(&first, &two).status.success());
    assert_eq!(
        fs::read(first.directory.path().join("received/two.txt")).unwrap(),
        b"from second"
    );
}
#[test]
fn auto_reject_sends_no_content() {
    let first = Receiver::start(false);
    let second = Receiver::start(true);
    first.pair(&second);
    let file = first.directory.path().join("denied.txt");
    fs::write(&file, b"must not arrive").unwrap();
    assert!(!first.send(&second, &file).status.success());
    assert_eq!(
        fs::read_dir(second.directory.path().join("received"))
            .unwrap()
            .count(),
        0
    );
}
#[test]
fn id_renders_versioned_qr() {
    let directory = tempfile::tempdir().unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_meshdrop"))
        .arg("--state-dir")
        .arg(directory.path())
        .arg("id")
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("meshdrop://127.0.0.1:9843#v=1&fp=")
    );
}
#[cfg(target_os = "linux")]
#[test]
fn large_transfer_keeps_receiver_child_below_96_mib() {
    let first = Receiver::start(false);
    let second = Receiver::start(false);
    first.pair(&second);
    let file = first.directory.path().join("large.bin");
    fs::File::create(&file)
        .unwrap()
        .set_len(128 * 1024 * 1024)
        .unwrap();
    assert!(first.send(&second, &file).status.success());
    let status = fs::read_to_string(format!("/proc/{}/status", second.process.id())).unwrap();
    let high_water: u64 = status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        high_water < 96 * 1024,
        "receiver child VmHWM {high_water} KiB"
    );
}

#[cfg(unix)]
#[test]
fn sigterm_stops_interactive_receiver_cleanly() {
    let mut receiver = Receiver::start(false);
    let signal = Command::new("kill")
        .args(["-TERM", &receiver.process.id().to_string()])
        .status()
        .unwrap();
    assert!(signal.success());
    assert!(receiver.process.wait().unwrap().success());
    assert_eq!(
        fs::read_dir(receiver.directory.path().join("received"))
            .unwrap()
            .count(),
        0
    );
}
