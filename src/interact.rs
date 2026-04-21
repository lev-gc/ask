use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::{IsTerminal, Write};
use std::time::Duration;

pub fn copy_first_command(first_cmd: Option<&str>) {
    let Some(cmd) = first_cmd else { return };
    if !std::io::stdout().is_terminal() {
        return;
    }
    let mut stderr = std::io::stderr();
    let _ = write!(
        stderr,
        "\x1b[2m[c] copy first command   [any] quit\x1b[0m "
    );
    let _ = stderr.flush();

    let pressed = read_single_key();

    // clear the prompt line
    let _ = write!(stderr, "\r\x1b[2K");
    let _ = stderr.flush();

    if matches!(pressed, Some('c') | Some('C') | Some('y') | Some('Y')) {
        match arboard::Clipboard::new().and_then(|mut c| c.set_text(cmd.to_string())) {
            Ok(()) => {
                let _ = writeln!(stderr, "\x1b[32mcopied:\x1b[0m {}", cmd);
            }
            Err(_) => {
                let _ = writeln!(
                    stderr,
                    "\x1b[33mclipboard unavailable. command:\x1b[0m {}",
                    cmd
                );
            }
        }
    }
}

fn read_single_key() -> Option<char> {
    if enable_raw_mode().is_err() {
        return None;
    }
    let result = loop {
        match event::poll(Duration::from_secs(30)) {
            Ok(true) => match event::read() {
                Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => match k.code {
                    KeyCode::Char(c) => break Some(c),
                    _ => break None,
                },
                Ok(_) => continue,
                Err(_) => break None,
            },
            _ => break None,
        }
    };
    let _ = disable_raw_mode();
    result
}
