//! Startup banner: block-strip `LLM-WIKI` wordmark drawn in Rust (no SVG).
//!
//! Truecolor paints only `█` when TTY and `NO_COLOR` unset; tagline lives in Clap `about` to avoid duplicate prose.

use std::fmt::Write;
use std::io::{self, IsTerminal};

const RESET: &str = "\x1b[0m";

/// Default logo fill: deep navy (`#0c2340`).
const LOGO_RGB: (u8, u8, u8) = (12, 35, 64);
/// `WIKI_LOGO_COLOR=black` → pure black blocks.
const ENV_LOGO_COLOR: &str = "WIKI_LOGO_COLOR";

fn logo_fill_rgb() -> (u8, u8, u8) {
    match std::env::var(ENV_LOGO_COLOR)
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Ok("black" | "0") => (0, 0, 0),
        Ok("blue" | "navy") => LOGO_RGB,
        _ => LOGO_RGB,
    }
}

/// Paint only full-block cells; spaces stay unstyled (terminal background).
fn paint_logo_line(line: &str, use_color: bool) -> String {
    if !use_color {
        return line.to_string();
    }
    let (r, g, b) = logo_fill_rgb();
    let mut out = String::with_capacity(line.len() * 2);
    for ch in line.chars() {
        if ch == '█' {
            let _ = write!(out, "\x1b[38;2;{};{};{}m{}{}", r, g, b, ch, RESET);
        } else {
            out.push(ch);
        }
    }
    out
}

/// Center `line` in `width` columns (display width == `line.len()` for our ASCII art).
fn center_line(line: &str, width: usize) -> String {
    let n = line.chars().count();
    if n >= width {
        return line.chars().take(width).collect();
    }
    let pad = width - n;
    let left = pad / 2;
    let right = pad - left;
    format!("{}{}{}", " ".repeat(left), line, " ".repeat(right))
}

/// Five-row strip-built letters: `LLM-WIKI` (monospace `█` + space).
fn llm_wiki_logo_rows() -> [String; 5] {
    const L: &[&str; 5] = &[
        "█    ",
        "█    ",
        "█    ",
        "█    ",
        "█████",
    ];
    const M: &[&str; 5] = &[
        "██   ██",
        "███ ███",
        "██ █ ██",
        "██   ██",
        "██   ██",
    ];
    const HY: &[&str; 5] = &[
        "   ",
        "   ",
        "   ",
        "███",
        "   ",
    ];
    const W: &[&str; 5] = &[
        "██     ██",
        "██     ██",
        "██  █  ██",
        "██ ███ ██",
        "███   ███",
    ];
    const I: &[&str; 5] = &[
        " █ ",
        " █ ",
        " █ ",
        " █ ",
        " █ ",
    ];
    const K: &[&str; 5] = &[
        "██   ██",
        "██  █  ",
        "████   ",
        "██  █  ",
        "██   ██",
    ];

    let parts: &[&[&str; 5]] = &[L, L, M, HY, W, I, K, I];
    let mut rows = [
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    ];
    for (i, p) in parts.iter().enumerate() {
        for r in 0..5 {
            if i > 0 {
                rows[r].push(' ');
            }
            rows[r].push_str(p[r]);
        }
    }
    rows
}

pub fn print_startup_banner() {
    let use_color = std::env::var_os("NO_COLOR").is_none() && io::stdout().is_terminal();

    let logo = llm_wiki_logo_rows();
    let logo_width = logo[0].chars().count() + 4;

    for ln in &logo {
        let centered = center_line(ln, logo_width);
        println!("{}", paint_logo_line(&centered, use_color));
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_rows_uniform_width() {
        let rows = llm_wiki_logo_rows();
        let w = rows[0].chars().count();
        for r in &rows {
            assert_eq!(r.chars().count(), w, "logo row width mismatch: {r:?}");
        }
        assert!(w <= 64, "logo should fit typical box: {w}");
    }
}
