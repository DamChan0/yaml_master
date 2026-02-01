use std::io::{self, Write};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};

pub fn copy_to_clipboard(text: &str) -> Result<()> {
    if osc52_copy(text).is_ok() {
        return Ok(());
    }
    if cfg!(target_os = "macos") {
        return command_copy("pbcopy", &[] as &[&str], text);
    }
    if cfg!(target_os = "windows") {
        return command_copy("clip.exe", &[] as &[&str], text);
    }
    if command_copy("wl-copy", &[], text).is_ok() {
        return Ok(());
    }
    if command_copy("xclip", &["-selection", "clipboard"], text).is_ok() {
        return Ok(());
    }
    if command_copy("xsel", &["--clipboard", "--input"], text).is_ok() {
        return Ok(());
    }
    Err(anyhow!("No clipboard command succeeded"))
}

fn osc52_copy(text: &str) -> Result<()> {
    let encoded = general_purpose::STANDARD.encode(text.as_bytes());
    let sequence = format!("\x1b]52;c;{}\x07", encoded);
    let mut stdout = io::stdout();
    stdout.write_all(sequence.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

fn command_copy(cmd: &str, args: &[&str], text: &str) -> Result<()> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
    }
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("Clipboard command failed"))
    }
}
