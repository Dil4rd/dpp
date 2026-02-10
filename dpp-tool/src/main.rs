//! dpp-tool — Fancy CLI for the full Apple DMG pipeline
//!
//! A cross-platform tool to explore DMG disk images end-to-end:
//! DMG → HFS+ → PKG → PBZX → files
//!
//! # Usage
//!
//! ```text
//! dpp-tool info <dmg>                                    Full pipeline overview
//! dpp-tool bench <dmg>                                   Benchmark pipeline stages
//!
//! dpp-tool dmg info <dmg>                                DMG format & compression stats
//! dpp-tool dmg ls <dmg>                                  List partitions
//! dpp-tool dmg cat <dmg> [partition-id]                  Raw partition data to stdout
//!
//! dpp-tool hfs info <dmg>                                HFS+ volume header
//! dpp-tool hfs ls <dmg> <path>                           List directory contents
//! dpp-tool hfs tree <dmg> [path]                         Browse filesystem tree
//! dpp-tool hfs cat <dmg> <path>                          Extract file to stdout
//! dpp-tool hfs stat <dmg> <path>                         File metadata
//! dpp-tool hfs find <dmg> [-name pat] [-type f|d|l]     Find files (default: *.pkg)
//!
//! dpp-tool pkg info <dmg> <pkg-path>                     Package stats
//! dpp-tool pkg ls <dmg> <pkg-path>                       List XAR contents
//! dpp-tool pkg find <dmg> <pkg-path> [-name p] [-type f|d|l]  Find entries
//! dpp-tool pkg cat <dmg> <pkg-path> <file>               XAR entry to stdout
//!
//! dpp-tool payload info <dmg> <pkg> <comp>               Payload (PBZX/CPIO) stats
//! dpp-tool payload ls <dmg> <pkg> <comp> [path]          List payload files
//! dpp-tool payload tree <dmg> <pkg> <comp> [path]        Browse payload tree
//! dpp-tool payload find <dmg> <pkg> <comp> [-name p] [-type f|d|l]  Find payload files
//! dpp-tool payload cat <dmg> <pkg> <comp> <file>         Extract payload file to stdout
//! ```

mod style;
mod pipeline;
mod cmd_dmg;
mod cmd_hfs;
mod cmd_pkg;
mod cmd_payload;
mod cmd_info;
mod cmd_bench;

use std::env;
use std::process;

use style::{RED, RESET, BOLD, DIM, GREEN};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    let result = match args[1].as_str() {
        "dmg" => cmd_dmg::run(&args[2..]),
        "hfs" => cmd_hfs::run(&args[2..]),
        "pkg" => cmd_pkg::run(&args[2..]),
        "payload" => cmd_payload::run(&args[2..]),
        "info" => cmd_info::run(&args[2..]),
        "bench" | "benchmark" => cmd_bench::run(&args[2..]),
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        _ => {
            eprintln!("{RED}Unknown command: {}{RESET}", args[1]);
            print_usage();
            process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("{RED}error:{RESET} {e}");
        process::exit(1);
    }
}

fn print_usage() {
    eprintln!(
        r#"
{BOLD}dpp-tool{RESET} — Apple DMG pipeline explorer

{DIM}Navigate the full stack: DMG → HFS+ → PKG → PBZX → files{RESET}

{BOLD}USAGE:{RESET}
    dpp-tool <COMMAND> [OPTIONS]

{BOLD}COMMANDS:{RESET}
    {GREEN}info{RESET}        <dmg>          Full pipeline overview
    {GREEN}bench{RESET}       <dmg>          Benchmark pipeline stages
    {GREEN}dmg{RESET}         ...            DMG (UDIF) container commands
    {GREEN}hfs{RESET}         ...            HFS+ filesystem commands
    {GREEN}pkg{RESET}         ...            PKG (XAR) archive commands
    {GREEN}payload{RESET}     ...            Component payload (PBZX/CPIO) commands

{BOLD}EXAMPLES:{RESET}
    dpp-tool info Kernel_Debug_Kit.dmg
    dpp-tool dmg ls Kernel_Debug_Kit.dmg
    dpp-tool hfs tree Kernel_Debug_Kit.dmg /Library
    dpp-tool hfs find Kernel_Debug_Kit.dmg -name "*.kext" -type d
    dpp-tool pkg ls Kernel_Debug_Kit.dmg /KernelDebugKit.pkg
    dpp-tool payload ls Kernel_Debug_Kit.dmg /path.pkg com.apple.pkg.KDK /

{DIM}Run dpp-tool <command> help for details{RESET}
"#
    );
}
