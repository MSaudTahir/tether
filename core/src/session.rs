//! A running scrcpy window.

use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::util::{command, read_in_background, text};
use crate::{CoreError, Result};

#[derive(uniffi::Object)]
pub struct Session {
    pid: u32,
    child: Mutex<Child>,
    output: Mutex<Option<[JoinHandle<Vec<u8>>; 2]>>,
    stopped: AtomicBool,
}

impl Session {
    pub fn spawn(scrcpy: &str, adb: &str, args: &[String]) -> Result<Arc<Session>> {
        let mut child = command(scrcpy).args(args).env("ADB", adb).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let output = [read_in_background(child.stdout.take()), read_in_background(child.stderr.take())];
        Ok(Arc::new(Session {
            pid: child.id(),
            child: Mutex::new(child),
            output: Mutex::new(Some(output)),
            stopped: AtomicBool::new(false),
        }))
    }

    pub fn finished(&self) -> bool {
        matches!(self.child.lock().unwrap().try_wait(), Ok(Some(_)))
    }
}

#[uniffi::export]
impl Session {
    /// The scrcpy process, e.g. to bring its window to the front.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Blocks until the window closes. Fails with scrcpy's errors if it didn't exit cleanly.
    pub fn wait(&self) -> Result<()> {
        let status = loop {
            if let Some(status) = self.child.lock().unwrap().try_wait()? {
                break status;
            }
            thread::sleep(Duration::from_millis(200));
        };
        if status.success() || self.stopped.load(Ordering::Relaxed) {
            return Ok(());
        }
        let output: String = self
            .output
            .lock()
            .unwrap()
            .take()
            .into_iter()
            .flatten()
            .map(|pipe| text(&pipe.join().unwrap_or_default()))
            .collect();
        let errors: Vec<&str> = output.lines().filter(|l| l.contains("ERROR")).collect();
        Err(CoreError::Failed(if errors.is_empty() { format!("scrcpy exited with {status}") } else { errors.join("\n") }))
    }

    /// Closes the window.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
        let _ = self.child.lock().unwrap().kill();
    }
}
