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
        // OSC 52: ask the terminal emulator to write `cmd` into the user's
        // system clipboard. Works over SSH / containers / Codespaces because
        // the protocol rides on the terminal byte stream — no display server,
        // no fork, no clipboard daemon needed.
        let encoded = base64_encode(cmd.as_bytes());
        let _ = write!(stderr, "\x1b]52;c;{}\x07", encoded);
        let _ = writeln!(stderr, "\x1b[32mcopied:\x1b[0m {}", cmd);
        let _ = stderr.flush();
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

fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut chunks = input.chunks_exact(3);
    for c in chunks.by_ref() {
        let n = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | (c[2] as u32);
        out.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
        out.push(CHARS[((n >> 6) & 0x3F) as usize] as char);
        out.push(CHARS[(n & 0x3F) as usize] as char);
    }
    let rem = chunks.remainder();
    match rem.len() {
        1 => {
            let n = (rem[0] as u32) << 16;
            out.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
            out.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let n = ((rem[0] as u32) << 16) | ((rem[1] as u32) << 8);
            out.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
            out.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
            out.push(CHARS[((n >> 6) & 0x3F) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::base64_encode;

    #[test]
    fn base64_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(b"df -h"), "ZGYgLWg=");
    }
}
