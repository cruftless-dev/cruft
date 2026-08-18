
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

const HIST_MAX: usize = 1000;

fn history_path() -> Option<PathBuf> {
    match std::env::var("CRUFT_REPL_HISTORY") {
        Ok(s) if s.is_empty() => None,
        Ok(s) => Some(PathBuf::from(s)),

        Err(_) => cruft::platform::user_home_dir().map(|h| h.join(".cruft_repl_history")),
    }
}

pub enum Input {
    Line(String),

    Interrupt,

    Eof,
}

struct RawGuard {
    saved: Option<String>,
}

impl RawGuard {
    fn enable() -> Self {
        let saved = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        if saved.is_some() {
            let _ = Command::new("stty")
                .args(["-icanon", "-echo", "-isig", "min", "1", "time", "0"])
                .stdin(Stdio::inherit())
                .status();
        }
        RawGuard { saved }
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        if let Some(s) = &self.saved {
            let _ = Command::new("stty").arg(s).stdin(Stdio::inherit()).status();
        }
    }
}

pub struct LineReader {
    tty: bool,
    _raw: Option<RawGuard>,
    history: Vec<String>,

    hist_path: Option<PathBuf>,

    preview: bool,
}

impl LineReader {
    pub fn new(tty: bool) -> Self {
        let raw = if tty { Some(RawGuard::enable()) } else { None };

        let preview = tty
            && std::env::var("CRUFT_REPL_PREVIEW")
                .map(|v| v != "0")
                .unwrap_or(true);

        let hist_path = if tty { history_path() } else { None };
        let history = hist_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|s| s.lines().map(String::from).collect::<Vec<_>>())
            .unwrap_or_default();
        LineReader {
            tty,
            _raw: raw,
            history,
            hist_path,
            preview,
        }
    }

    pub fn add_history(&mut self, line: &str) {

        if !line.trim().is_empty() && self.history.last().map(|l| l != line).unwrap_or(true) {
            self.history.push(line.to_string());
        }
    }

    pub fn read_line(
        &mut self,
        prompt: &str,
        complete: &mut dyn FnMut(&str, usize) -> (String, Vec<String>),
    ) -> Input {
        if self.tty {
            self.read_raw(prompt, complete)
        } else {
            self.read_cooked(prompt)
        }
    }

    fn read_cooked(&mut self, prompt: &str) -> Input {

        print!("{}", prompt);
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(0) => Input::Eof,
            Ok(_) => Input::Line(line.trim_end_matches(['\n', '\r']).to_string()),
            Err(_) => Input::Eof,
        }
    }

    fn read_raw(
        &mut self,
        prompt: &str,
        complete: &mut dyn FnMut(&str, usize) -> (String, Vec<String>),
    ) -> Input {
        let mut buf: Vec<char> = Vec::new();
        let mut cursor: usize = 0;

        let mut hist_idx = self.history.len();
        let mut draft: Vec<char> = Vec::new();

        let mut ghost = String::new();
        redraw(prompt, &buf, cursor, &ghost);

        let mut stdin = std::io::stdin();
        let mut byte = [0u8; 1];
        loop {
            if stdin.read(&mut byte).unwrap_or(0) == 0 {
                println!();
                return Input::Eof;
            }
            match byte[0] {
                b'\r' | b'\n' => {
                    print!("\r\n");
                    let _ = std::io::stdout().flush();
                    return Input::Line(buf.iter().collect());
                }
                0x03 => {

                    print!("\r\n");
                    let _ = std::io::stdout().flush();
                    return Input::Interrupt;
                }
                0x04 => {

                    if buf.is_empty() {
                        println!();
                        return Input::Eof;
                    }
                }
                0x7f | 0x08 => {

                    if cursor > 0 {
                        cursor -= 1;
                        buf.remove(cursor);
                        ghost = compute_ghost(self.preview, &buf, cursor, complete);
                        redraw(prompt, &buf, cursor, &ghost);
                    }
                }
                0x01 => {
                    cursor = 0;
                    ghost.clear();
                    redraw(prompt, &buf, cursor, &ghost);
                }
                0x05 => {
                    cursor = buf.len();
                    ghost.clear();
                    redraw(prompt, &buf, cursor, &ghost);
                }
                0x0b => {
                    buf.truncate(cursor);
                    ghost = compute_ghost(self.preview, &buf, cursor, complete);
                    redraw(prompt, &buf, cursor, &ghost);
                }
                0x15 => {
                    buf.drain(0..cursor);
                    cursor = 0;
                    ghost = compute_ghost(self.preview, &buf, cursor, complete);
                    redraw(prompt, &buf, cursor, &ghost);
                }
                0x09 => {

                    let line: String = buf.iter().collect();
                    let (prefix, cands) = complete(&line, cursor);
                    let plen = prefix.chars().count();
                    let insert = |buf: &mut Vec<char>, cursor: &mut usize, s: &str| {
                        for c in s.chars() {
                            buf.insert(*cursor, c);
                            *cursor += 1;
                        }
                    };
                    if cands.len() == 1 {
                        let suffix: String = cands[0].chars().skip(plen).collect();
                        insert(&mut buf, &mut cursor, &suffix);
                    } else if cands.len() > 1 {
                        let lcp = longest_common_prefix(&cands);
                        if lcp.chars().count() > plen {
                            let suffix: String = lcp.chars().skip(plen).collect();
                            insert(&mut buf, &mut cursor, &suffix);
                        } else {

                            print!("\r\n{}\r\n", cands.join("  "));
                        }
                    }
                    ghost = compute_ghost(self.preview, &buf, cursor, complete);
                    redraw(prompt, &buf, cursor, &ghost);
                }
                0x1b => {

                    let mut b2 = [0u8; 1];
                    let mut b3 = [0u8; 1];
                    if stdin.read(&mut b2).unwrap_or(0) == 0 {
                        continue;
                    }
                    if b2[0] != b'[' && b2[0] != b'O' {
                        continue;
                    }
                    if stdin.read(&mut b3).unwrap_or(0) == 0 {
                        continue;
                    }
                    match b3[0] {
                        b'D' if cursor > 0 => {
                            cursor -= 1;
                            ghost.clear();
                            redraw(prompt, &buf, cursor, &ghost);
                        }
                        b'C' => {

                            if cursor == buf.len() && !ghost.is_empty() {
                                let g = std::mem::take(&mut ghost);
                                for c in g.chars() {
                                    buf.insert(cursor, c);
                                    cursor += 1;
                                }
                                ghost = compute_ghost(self.preview, &buf, cursor, complete);
                                redraw(prompt, &buf, cursor, &ghost);
                            } else if cursor < buf.len() {
                                cursor += 1;
                                ghost.clear();
                                redraw(prompt, &buf, cursor, &ghost);
                            }
                        }
                        b'H' => {
                            cursor = 0;
                            ghost.clear();
                            redraw(prompt, &buf, cursor, &ghost);
                        }
                        b'F' => {

                            if cursor == buf.len() && !ghost.is_empty() {
                                let g = std::mem::take(&mut ghost);
                                for c in g.chars() {
                                    buf.insert(cursor, c);
                                    cursor += 1;
                                }
                                ghost = compute_ghost(self.preview, &buf, cursor, complete);
                            } else {
                                cursor = buf.len();
                                ghost.clear();
                            }
                            redraw(prompt, &buf, cursor, &ghost);
                        }
                        b'A' => {

                            if hist_idx > 0 {
                                if hist_idx == self.history.len() {
                                    draft = buf.clone();
                                }
                                hist_idx -= 1;
                                buf = self.history[hist_idx].chars().collect();
                                cursor = buf.len();
                                ghost.clear();
                                redraw(prompt, &buf, cursor, &ghost);
                            }
                        }
                        b'B' => {

                            if hist_idx < self.history.len() {
                                hist_idx += 1;
                                buf = if hist_idx == self.history.len() {
                                    draft.clone()
                                } else {
                                    self.history[hist_idx].chars().collect()
                                };
                                cursor = buf.len();
                                ghost.clear();
                                redraw(prompt, &buf, cursor, &ghost);
                            }
                        }
                        _ => {}
                    }
                }
                b if b >= 0x20 => {

                    let ch = read_utf8_char(b, &mut stdin);
                    if let Some(c) = ch {
                        buf.insert(cursor, c);
                        cursor += 1;
                        ghost = compute_ghost(self.preview, &buf, cursor, complete);
                        redraw(prompt, &buf, cursor, &ghost);
                    }
                }
                _ => {}
            }
        }
    }
}

impl Drop for LineReader {
    fn drop(&mut self) {

        if let Some(p) = &self.hist_path {
            let start = self.history.len().saturating_sub(HIST_MAX);
            let _ = std::fs::write(p, self.history[start..].join("\n"));
        }
    }
}

fn longest_common_prefix(cands: &[String]) -> String {
    if cands.is_empty() {
        return String::new();
    }
    let first: Vec<char> = cands[0].chars().collect();
    let mut len = first.len();
    for c in &cands[1..] {
        let cc: Vec<char> = c.chars().collect();
        let mut i = 0;
        while i < len && i < cc.len() && first[i] == cc[i] {
            i += 1;
        }
        len = i;
    }
    first[..len].iter().collect()
}

fn redraw(prompt: &str, buf: &[char], cursor: usize, ghost: &str) {
    let line: String = buf.iter().collect();
    print!("\r\x1b[K{}{}", prompt, line);

    let show_ghost = !ghost.is_empty() && cursor == buf.len();
    if show_ghost {
        print!("\x1b[90m{}\x1b[0m", ghost);
    }

    let printed_end = buf.len() + if show_ghost { ghost.chars().count() } else { 0 };
    if printed_end > cursor {
        print!("\x1b[{}D", printed_end - cursor);
    }
    let _ = std::io::stdout().flush();
}

fn compute_ghost(
    preview: bool,
    buf: &[char],
    cursor: usize,
    complete: &mut dyn FnMut(&str, usize) -> (String, Vec<String>),
) -> String {
    if !preview || cursor != buf.len() || buf.is_empty() {
        return String::new();
    }
    let line: String = buf.iter().collect();
    let (prefix, cands) = complete(&line, cursor);
    if cands.is_empty() {
        return String::new();
    }
    let plen = prefix.chars().count();
    let lcp = if cands.len() == 1 {
        cands[0].clone()
    } else {
        longest_common_prefix(&cands)
    };
    if lcp.chars().count() > plen {
        lcp.chars().skip(plen).collect()
    } else {
        String::new()
    }
}

fn read_utf8_char(first: u8, stdin: &mut std::io::Stdin) -> Option<char> {
    let n = if first < 0x80 {
        0
    } else if first >> 5 == 0b110 {
        1
    } else if first >> 4 == 0b1110 {
        2
    } else if first >> 3 == 0b11110 {
        3
    } else {
        return None;
    };
    let mut bytes = vec![first];
    for _ in 0..n {
        let mut b = [0u8; 1];
        if stdin.read(&mut b).ok()? == 0 {
            return None;
        }
        bytes.push(b[0]);
    }
    std::str::from_utf8(&bytes).ok()?.chars().next()
}
