//! Startup banner: ANSI Shadow style `LLM-WIKI` wordmark with gradient coloring.

use std::io::{self, IsTerminal};

const RESET: &str = "\x1b[0m";

fn banner_ascii() -> &'static str {
    r#"
    ██╗     ██╗     ███╗   ███╗    ██╗    ██╗██╗██╗  ██╗██╗
    ██║     ██║     ████╗ ████║    ██║    ██║██║██║ ██╔╝██║
    ██║     ██║     ██╔████╔██║    ██║ █╗ ██║██║█████╔╝ ██║
    ██║     ██║     ██║╚██╔╝██║    ██║███╗██║██║██╔═██╗ ██║
    ███████╗███████╗██║ ╚═╝ ██║    ╚███╔███╔╝██║██║  ██╗██║
    ╚══════╝╚══════╝╚═╝     ╚═╝     ╚══╝╚══╝ ╚═╝╚═╝  ╚═╝╚═╝

          Knowledge Kernel  •  RRF Search  •  MCP Ready
"#
}

pub fn print_startup_banner() {
    let use_color = std::env::var_os("NO_COLOR").is_none() && io::stdout().is_terminal();

    if !use_color {
        print!("{}", banner_ascii());
        return;
    }

    let gradient: &[u8] = &[39, 38, 44, 43, 49, 48, 83, 118, 154];

    for (i, line) in banner_ascii().lines().enumerate() {
        if line.trim().is_empty() {
            println!();
            continue;
        }
        let c = gradient[i % gradient.len()];
        println!("\x1b[38;5;{c}m{line}{RESET}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_rows_uniform_width() {
        let lines: Vec<&str> = banner_ascii()
            .lines()
            .filter(|l| l.contains('█'))
            .collect();
        assert!(!lines.is_empty());
        let w = lines[0].chars().count();
        for l in &lines {
            assert_eq!(
                l.chars().count(),
                w,
                "logo row width mismatch: {l:?}"
            );
        }
        assert!(w <= 80, "logo should fit typical terminal: {w}");
    }
}
