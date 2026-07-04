//! FCS CLI — compile and inspect FCS chart files.

use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;

#[derive(Parser)]
#[command(name = "fcs", about = "FCS chart compiler and inspector")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile a .fcs source file to .fcbc bytecode
    Compile {
        input: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Dump .fcbc bytecode as hex
    Dump { input: String },
    /// Check a .fcs file for errors without writing output
    Check { input: String },
    /// Show chart metadata from .fcs or .fcbc
    Info { input: String },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Compile { input, output } => cmd_compile(&input, output.as_deref()),
        Commands::Dump { input } => cmd_dump(&input),
        Commands::Check { input } => cmd_check(&input),
        Commands::Info { input } => cmd_info(&input),
    }
}

fn cmd_compile(input: &str, output: Option<&str>) {
    let src = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => { eprintln!("error reading '{}': {}", input, e); return; }
    };

    let (_, doc) = match fcs_core::parser::parse_document(&src) {
        Ok(r) => r,
        Err(e) => { eprintln!("parse error: {:?}", e); return; }
    };

    match fcs_core::compiler::compile(&doc) {
        Ok(file) => {
            let out_path = output.unwrap_or_else(|| {
                // boxing needed because with_extension returns &Path
                let p = Path::new(input).with_extension("fcbc");
                // leak is fine for CLI — process exits immediately
                Box::leak(Box::new(p)).to_str().unwrap_or("output.fcbc")
            });
            let bytes = file.to_bytes();
            if let Err(e) = fs::write(out_path, &bytes) {
                eprintln!("error writing '{}': {}", out_path, e);
            } else {
                println!("Compiled '{}' -> '{}' ({} bytes)", input, out_path, bytes.len());
            }
        }
        Err(diag) => {
            eprintln!("{}", diag);
        }
    }
}

fn cmd_check(input: &str) {
    let src = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => { eprintln!("error reading '{}': {}", input, e); return; }
    };

    match fcs_core::parser::parse_document(&src) {
        Ok((_, doc)) => {
            match fcs_core::compiler::compile(&doc) {
                Ok(_) => println!("'{}' is valid.", input),
                Err(diag) => eprintln!("{}", diag),
            }
        }
        Err(e) => eprintln!("parse error: {:?}", e),
    }
}

fn cmd_dump(input: &str) {
    let data = match fs::read(input) {
        Ok(d) => d,
        Err(e) => { eprintln!("error reading '{}': {}", input, e); return; }
    };

    if data.len() < 4 || &data[0..4] != b"FCSB" {
        eprintln!("'{}' is not a valid .fcbc file (bad magic)", input);
        return;
    }

    println!("File: {} ({} bytes)", input, data.len());
    println!("Magic: FCSB (valid)");

    // Dump header
    if data.len() >= 28 {
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let flags = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let st_off = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let st_size = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
        let cp_off = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        let cp_size = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);

        println!("Version: {}", version);
        println!("Flags: 0x{:08X} (shader={}, expr={})", flags,
            flags & 1 != 0, flags & 2 != 0);
        println!("StringTable: offset={} size={}", st_off, st_size);
        println!("ConstPool:   offset={} size={}", cp_off, cp_size);
    }

    // Dump hex of first 128 bytes
    let limit = data.len().min(128);
    for (i, chunk) in data[..limit].chunks(16).enumerate() {
        print!("{:08X}  ", i * 16);
        for b in chunk { print!("{:02X} ", b); }
        println!();
    }
    if data.len() > 128 {
        println!("... ({} more bytes)", data.len() - 128);
    }
}

fn cmd_info(input: &str) {
    let src = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => { eprintln!("error reading '{}': {}", input, e); return; }
    };

    match fcs_core::parser::parse_document(&src) {
        Ok((_, doc)) => {
            println!("Name:      {}", doc.meta.name);
            println!("Artists:   {}", doc.meta.artists.join(", "));
            println!("Charters:  {}", doc.meta.charters.join(", "));
            println!("Offset:    {}ms", doc.meta.offset);
            println!("Version:   {}", doc.meta.version);
            println!("Lines:     {}", doc.judgelines.lines.len());
            let total_notes: usize = doc.judgelines.lines.iter()
                .map(|l| l.notes.instances.len()).sum();
            println!("Notes:     {}", total_notes);
            let bpm_count = doc.master_timeline.entries.len();
            println!("BPM stops: {}", bpm_count);
        }
        Err(e) => eprintln!("parse error: {:?}", e),
    }
}
