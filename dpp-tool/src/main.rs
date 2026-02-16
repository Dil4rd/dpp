//! dpp-tool — Fancy CLI for the full Apple DMG pipeline
//!
//! A cross-platform tool to explore DMG disk images end-to-end:
//! DMG → HFS+/APFS → PKG → PBZX → files

mod cmd_apfs;
mod cmd_bench;
mod cmd_dmg;
mod cmd_fs;
mod cmd_hfs;
mod cmd_info;
mod cmd_payload;
mod cmd_pkg;
mod pipeline;
mod style;

use std::io;
use std::path::PathBuf;
use std::process;

use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};

// ── Top-level CLI ────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "dpp-tool",
    about = "Apple DMG pipeline explorer — navigate DMG → HFS+/APFS → PKG → PBZX → files",
    version,
    after_help = "\
EXAMPLES:
    dpp-tool info Kernel_Debug_Kit.dmg
    dpp-tool --in-memory fs info small.dmg
    dpp-tool dmg ls Kernel_Debug_Kit.dmg
    dpp-tool fs tree Kernel_Debug_Kit.dmg /Library
    dpp-tool fs find Kernel_Debug_Kit.dmg -n \"*.kext\" -t d
    dpp-tool pkg ls Kernel_Debug_Kit.dmg /KernelDebugKit.pkg
    dpp-tool payload ls Kernel_Debug_Kit.dmg /path.pkg com.apple.pkg.KDK /"
)]
struct Cli {
    /// Extract partitions via temp file (default, low memory)
    #[arg(long, global = true, conflicts_with = "in_memory")]
    temp_file: bool,

    /// Buffer partitions in memory (faster for small DMGs)
    #[arg(long, global = true)]
    in_memory: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Full pipeline overview
    Info {
        /// Path to the DMG file
        dmg: PathBuf,
    },

    /// Benchmark pipeline stages
    #[command(alias = "benchmark")]
    Bench {
        /// Path to the DMG file
        dmg: PathBuf,
    },

    /// DMG (UDIF) container commands
    Dmg {
        #[command(subcommand)]
        command: DmgCommand,
    },

    /// Filesystem commands (auto-detect HFS+/APFS)
    Fs {
        #[command(subcommand)]
        command: FsCommand,
    },

    /// HFS+ filesystem commands
    Hfs {
        #[command(subcommand)]
        command: HfsCommand,
    },

    /// APFS filesystem commands
    Apfs {
        #[command(subcommand)]
        command: ApfsCommand,
    },

    /// PKG (XAR) archive commands
    Pkg {
        #[command(subcommand)]
        command: PkgCommand,
    },

    /// Component payload (PBZX/CPIO) commands
    Payload {
        #[command(subcommand)]
        command: PayloadCommand,
    },

    /// Generate shell completions
    #[command(hide = true)]
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

// ── DMG subcommands ──────────────────────────────────────────────────────

#[derive(Subcommand)]
enum DmgCommand {
    /// Format & compression stats
    Info {
        /// Path to the DMG file
        dmg: PathBuf,
    },
    /// List partitions
    Ls {
        /// Path to the DMG file
        dmg: PathBuf,
    },
    /// Raw partition data to stdout
    Cat {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Partition ID (default: main partition)
        partition_id: Option<i32>,
    },
}

// ── Filesystem subcommands (fs, hfs, apfs share a shape) ────────────────

#[derive(Args)]
struct FindArgs {
    /// Glob pattern to match file names
    #[arg(short, long)]
    name: Option<String>,

    /// Filter by type: f (file), d (directory), l (symlink)
    #[arg(short = 't', long = "type")]
    file_type: Option<char>,
}

#[derive(Subcommand)]
enum FsCommand {
    /// Volume info (auto-detect HFS+/APFS)
    Info {
        /// Path to the DMG file
        dmg: PathBuf,
    },
    /// List directory contents
    Ls {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to list
        path: String,
    },
    /// Browse filesystem tree
    Tree {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Root path for tree (default: /)
        path: Option<String>,
        /// Maximum tree depth
        #[arg(short, long, default_value_t = 3)]
        depth: usize,
    },
    /// Extract file to stdout
    Cat {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to extract
        path: String,
    },
    /// File metadata
    Stat {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to stat
        path: String,
    },
    /// Find files (default: *.pkg)
    Find {
        /// Path to the DMG file
        dmg: PathBuf,
        #[command(flatten)]
        args: FindArgs,
    },
}

#[derive(Subcommand)]
enum HfsCommand {
    /// HFS+ volume header
    Info {
        /// Path to the DMG file
        dmg: PathBuf,
    },
    /// List directory contents
    Ls {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to list
        path: String,
    },
    /// Browse filesystem tree
    Tree {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Root path for tree (default: /)
        path: Option<String>,
        /// Maximum tree depth
        #[arg(short, long, default_value_t = 3)]
        depth: usize,
    },
    /// Extract file to stdout
    Cat {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to extract
        path: String,
    },
    /// File metadata (CNID, perms, dates, forks)
    Stat {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to stat
        path: String,
    },
    /// Find files (default: *.pkg)
    Find {
        /// Path to the DMG file
        dmg: PathBuf,
        #[command(flatten)]
        args: FindArgs,
    },
}

#[derive(Subcommand)]
enum ApfsCommand {
    /// APFS volume info
    Info {
        /// Path to the DMG file
        dmg: PathBuf,
    },
    /// List directory contents
    Ls {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to list
        path: String,
    },
    /// Browse filesystem tree
    Tree {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Root path for tree (default: /)
        path: Option<String>,
        /// Maximum tree depth
        #[arg(short, long, default_value_t = 3)]
        depth: usize,
    },
    /// Extract file to stdout
    Cat {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to extract
        path: String,
    },
    /// File metadata (OID, perms, dates)
    Stat {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to stat
        path: String,
    },
    /// Find files (default: *.pkg)
    Find {
        /// Path to the DMG file
        dmg: PathBuf,
        #[command(flatten)]
        args: FindArgs,
    },
}

// ── PKG subcommands ──────────────────────────────────────────────────────

#[derive(Subcommand)]
enum PkgCommand {
    /// Package stats
    Info {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to the .pkg file
        pkg_path: String,
    },
    /// List XAR contents
    Ls {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to the .pkg file
        pkg_path: String,
    },
    /// Find entries (default: *.pkg components)
    Find {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to the .pkg file
        pkg_path: String,
        #[command(flatten)]
        args: FindArgs,
    },
    /// XAR entry to stdout
    Cat {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to the .pkg file
        pkg_path: String,
        /// File within the XAR to extract
        file: String,
    },
}

// ── Payload subcommands ──────────────────────────────────────────────────

#[derive(Subcommand)]
enum PayloadCommand {
    /// Payload (PBZX/CPIO) stats
    Info {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to the .pkg file
        pkg_path: String,
        /// Component identifier
        component: String,
    },
    /// List payload files
    Ls {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to the .pkg file
        pkg_path: String,
        /// Component identifier
        component: String,
        /// Path within the payload (default: /)
        path: Option<String>,
    },
    /// Browse payload tree
    Tree {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to the .pkg file
        pkg_path: String,
        /// Component identifier
        component: String,
        /// Root path for tree (default: /)
        path: Option<String>,
        /// Maximum tree depth
        #[arg(short, long, default_value_t = 3)]
        depth: usize,
    },
    /// Find payload files
    Find {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to the .pkg file
        pkg_path: String,
        /// Component identifier
        component: String,
        #[command(flatten)]
        args: FindArgs,
    },
    /// Extract payload file to stdout
    Cat {
        /// Path to the DMG file
        dmg: PathBuf,
        /// Filesystem path to the .pkg file
        pkg_path: String,
        /// Component identifier
        component: String,
        /// File within the payload to extract
        file: String,
    },
}

// ── main ─────────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    // Initialize color support
    style::init_color(cli.no_color);

    let mode = if cli.in_memory {
        dpp::ExtractMode::InMemory
    } else {
        dpp::ExtractMode::default()
    };

    let result = match cli.command {
        Command::Info { dmg } => cmd_info::run(&dmg, mode),
        Command::Bench { dmg } => cmd_bench::run(&dmg, mode),
        Command::Dmg { command } => cmd_dmg::run(command, mode),
        Command::Fs { command } => cmd_fs::run(command, mode),
        Command::Hfs { command } => cmd_hfs::run(command, mode),
        Command::Apfs { command } => cmd_apfs::run(command, mode),
        Command::Pkg { command } => cmd_pkg::run(command, mode),
        Command::Payload { command } => cmd_payload::run(command, mode),
        Command::Completions { shell } => {
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "dpp-tool", &mut io::stdout());
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("{}error:{} {e}", style::red(), style::reset());
        process::exit(1);
    }
}
