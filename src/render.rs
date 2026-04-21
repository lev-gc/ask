use std::io::{IsTerminal, Write};

pub struct Renderer {
    buf: String,
    first_cmd: Option<String>,
    use_color: bool,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            first_cmd: None,
            use_color: std::io::stdout().is_terminal(),
        }
    }

    pub fn feed(&mut self, chunk: &str) {
        self.buf.push_str(chunk);
        // emit complete lines
        while let Some(pos) = self.buf.find('\n') {
            let line: String = self.buf.drain(..=pos).collect();
            let line_no_nl = line.trim_end_matches('\n').to_string();
            self.print_line(&line_no_nl);
        }
    }

    pub fn finish(&mut self) {
        if !self.buf.is_empty() {
            let rest = std::mem::take(&mut self.buf);
            self.print_line(&rest);
            println!();
        }
        let _ = std::io::stdout().flush();
    }

    pub fn first_command(&self) -> Option<&str> {
        self.first_cmd.as_deref()
    }

    fn print_line(&mut self, line: &str) {
        let mut out = std::io::stdout().lock();
        if let Some(cmd) = line.strip_prefix("$ ") {
            if self.first_cmd.is_none() {
                self.first_cmd = Some(cmd.to_string());
            }
            if self.use_color {
                let _ = writeln!(out, "\x1b[1;36m$\x1b[0m \x1b[1m{}\x1b[0m", cmd);
            } else {
                let _ = writeln!(out, "$ {}", cmd);
            }
        } else if line.is_empty() {
            let _ = writeln!(out);
        } else if self.use_color {
            let _ = writeln!(out, "\x1b[2m{}\x1b[0m", line);
        } else {
            let _ = writeln!(out, "{}", line);
        }
        let _ = out.flush();
    }
}
