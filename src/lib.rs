use std::error::Error as StdError;
use std::io;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread::JoinHandle;

pub struct ShellProcess {
	process: Child,
	stdin_thread: JoinHandle<()>,

	tx: mpsc::SyncSender<String>,
	rx: mpsc::Receiver<io::Result<()>>,
}

impl ShellProcess {
	pub fn new(hostname: &str) -> Result<Self, Box<dyn StdError>> {
		let mut ssh_command = Command::new("ssh.exe");
		ssh_command
			.args([hostname, "/bin/bash", "--noediting"])
			.stdout(Stdio::inherit())
			.stderr(Stdio::inherit())
			.stdin(Stdio::piped());

		let mut process = ssh_command.spawn()?;
		let mut ssh_stdin = process
			.stdin
			.take()
			.ok_or(io::Error::new(io::ErrorKind::Other, "Failed to open stdin"))?;

		let (tx, rx) = mpsc::sync_channel::<String>(1);
		let (tx2, rx2) = mpsc::sync_channel::<io::Result<()>>(1);

		let stdin_thread: JoinHandle<()> = std::thread::spawn(move || {
			while let Ok(v) = rx.recv() {
				if let result @ Err(_) = ssh_stdin.write_all(v.as_bytes()) {
					tx2.send(result).unwrap();
				}
				if let result @ Err(_) = ssh_stdin.write_all(b"\n") {
					tx2.send(result).unwrap();
				}
				if let result @ Err(_) = ssh_stdin.flush() {
					tx2.send(result).unwrap();
				}
				tx2.send(Ok(())).unwrap();
			}
		});

		Ok(ShellProcess {
			process,
			stdin_thread,
			tx,
			rx: rx2,
		})
	}

	pub fn send_command(&self, cmd: impl AsRef<str>) -> Result<(), Box<dyn StdError>> {
		self.tx.send(cmd.as_ref().to_string())?;
		let cmd_write_result: io::Result<()> = self.rx.recv()?;
		Ok(cmd_write_result?)
	}

	pub fn close(mut self) -> Result<(), Box<dyn StdError>> {
		// Close the stdin by closing this channel which will cause the stdin thread to exit
		drop(self.tx);
		self.process.wait()?;
		self.stdin_thread.join().unwrap();
		Ok(())
	}
}
