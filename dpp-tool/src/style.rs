use std::io::{self, Write};

// ── ANSI styling ─────────────────────────────────────────────────────────

pub(crate) const RESET: &str = "\x1b[0m";
pub(crate) const BOLD: &str = "\x1b[1m";
pub(crate) const DIM: &str = "\x1b[2m";
pub(crate) const GREEN: &str = "\x1b[32m";
pub(crate) const CYAN: &str = "\x1b[36m";
pub(crate) const YELLOW: &str = "\x1b[33m";
pub(crate) const BLUE: &str = "\x1b[34m";
pub(crate) const RED: &str = "\x1b[31m";
pub(crate) const WHITE: &str = "\x1b[37m";

// ── Box-drawing ──────────────────────────────────────────────────────────

pub(crate) const PIPE: &str = "│";
pub(crate) const TEE: &str = "├──";
pub(crate) const ELBOW: &str = "└──";
pub(crate) const DASH_LINE: &str = "────────────────────────────────────────────────────────────";
pub(crate) const DOUBLE_LINE: &str = "════════════════════════════════════════════════════════════";

// ── Formatting ───────────────────────────────────────────────────────────

pub(crate) fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

pub(crate) fn format_duration(d: std::time::Duration) -> String {
    let ms = d.as_secs_f64() * 1000.0;
    if ms >= 1000.0 {
        format!("{:.2}s", d.as_secs_f64())
    } else if ms >= 1.0 {
        format!("{:.1}ms", ms)
    } else {
        format!("{:.0}us", ms * 1000.0)
    }
}

pub(crate) fn format_commas(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut result = Vec::new();
    for (i, &b) in bytes.iter().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(b',');
        }
        result.push(b);
    }
    result.reverse();
    String::from_utf8(result).unwrap()
}

pub(crate) fn mode_string(mode: u16) -> String {
    let file_type = match mode >> 12 {
        0o12 => 'l',
        0o04 => 'd',
        _ => '-',
    };
    let perms = [
        if mode & 0o400 != 0 { 'r' } else { '-' },
        if mode & 0o200 != 0 { 'w' } else { '-' },
        if mode & 0o100 != 0 { 'x' } else { '-' },
        if mode & 0o040 != 0 { 'r' } else { '-' },
        if mode & 0o020 != 0 { 'w' } else { '-' },
        if mode & 0o010 != 0 { 'x' } else { '-' },
        if mode & 0o004 != 0 { 'r' } else { '-' },
        if mode & 0o002 != 0 { 'w' } else { '-' },
        if mode & 0o001 != 0 { 'x' } else { '-' },
    ];
    format!("{}{}", file_type, perms.iter().collect::<String>())
}

pub(crate) fn kind_icon(kind: hfsplus::EntryKind) -> &'static str {
    match kind {
        hfsplus::EntryKind::Directory => "dir",
        hfsplus::EntryKind::File => "   ",
        hfsplus::EntryKind::Symlink => "lnk",
    }
}

pub(crate) fn kind_color(kind: hfsplus::EntryKind) -> &'static str {
    match kind {
        hfsplus::EntryKind::Directory => BLUE,
        hfsplus::EntryKind::File => WHITE,
        hfsplus::EntryKind::Symlink => CYAN,
    }
}

pub(crate) fn apfs_kind_icon(kind: apfs::EntryKind) -> &'static str {
    match kind {
        apfs::EntryKind::Directory => "dir",
        apfs::EntryKind::File => "   ",
        apfs::EntryKind::Symlink => "lnk",
    }
}

pub(crate) fn apfs_kind_color(kind: apfs::EntryKind) -> &'static str {
    match kind {
        apfs::EntryKind::Directory => BLUE,
        apfs::EntryKind::File => WHITE,
        apfs::EntryKind::Symlink => CYAN,
    }
}

/// Simple glob matching supporting `*` (any sequence) and `?` (any single char).
/// Case-insensitive to match HFS+ default behavior.
pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().flat_map(|c| c.to_lowercase()).collect();
    let txt: Vec<char> = text.chars().flat_map(|c| c.to_lowercase()).collect();
    let (mut pi, mut ti) = (0, 0);
    let (mut star_pi, mut star_ti) = (usize::MAX, 0);

    while ti < txt.len() {
        if pi < pat.len() && (pat[pi] == '?' || pat[pi] == txt[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pat.len() && pat[pi] == '*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    while pi < pat.len() && pat[pi] == '*' {
        pi += 1;
    }
    pi == pat.len()
}

// ── UI output ────────────────────────────────────────────────────────────

pub(crate) fn header(title: &str) {
    println!();
    println!("  {BOLD}{title}{RESET}");
    println!("  {DIM}{DOUBLE_LINE}{RESET}");
}

pub(crate) fn section(title: &str) {
    println!();
    println!("  {CYAN}{BOLD}{title}{RESET}");
    println!("  {DIM}{DASH_LINE}{RESET}");
}

pub(crate) fn kv(key: &str, value: &str) {
    println!("  {DIM}{key:<24}{RESET} {value}");
}

pub(crate) fn kv_highlight(key: &str, value: &str) {
    println!("  {DIM}{key:<24}{RESET} {BOLD}{GREEN}{value}{RESET}");
}

pub(crate) fn spinner_msg(msg: &str) {
    eprint!("  {DIM}{YELLOW}>{RESET} {msg}...");
    io::stderr().flush().ok();
}

pub(crate) fn spinner_done(extra: &str) {
    eprintln!(" {GREEN}done{RESET}{DIM}{extra}{RESET}");
}
