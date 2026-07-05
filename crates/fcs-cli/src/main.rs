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
    /// Convert a Phigros chart (PGR/RPE/PEC) to .fcs
    #[command(name = "convert")]
    Convert {
        input: String,
        #[arg(short, long)]
        output: Option<String>,
        #[arg(short, long)]
        format: Option<String>,
        /// Reverse: convert .fcs to target format (pgr, rpe, pec)
        #[arg(long = "to")]
        to_format: Option<String>,
        /// PGR version for reverse conversion (1 or 3, default 3)
        #[arg(long = "pgr-version", default_value = "3")]
        pgr_version: i32,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Compile { input, output } => cmd_compile(&input, output.as_deref()),
        Commands::Dump { input } => cmd_dump(&input),
        Commands::Check { input } => cmd_check(&input),
        Commands::Info { input } => cmd_info(&input),
        Commands::Convert {
            input,
            output,
            format,
            to_format,
            pgr_version,
        } => cmd_convert(
            &input,
            output.as_deref(),
            format.as_deref(),
            to_format.as_deref(),
            pgr_version,
        ),
    }
}

fn cmd_compile(input: &str, output: Option<&str>) {
    let src = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading '{}': {}", input, e);
            return;
        }
    };

    let (_, doc) = match fcs_core::parser::parse_document(&src) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("parse error: {:?}", e);
            return;
        }
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
                println!(
                    "Compiled '{}' -> '{}' ({} bytes)",
                    input,
                    out_path,
                    bytes.len()
                );
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
        Err(e) => {
            eprintln!("error reading '{}': {}", input, e);
            return;
        }
    };

    match fcs_core::parser::parse_document(&src) {
        Ok((_, doc)) => match fcs_core::compiler::compile(&doc) {
            Ok(_) => println!("'{}' is valid.", input),
            Err(diag) => eprintln!("{}", diag),
        },
        Err(e) => eprintln!("parse error: {:?}", e),
    }
}

fn cmd_dump(input: &str) {
    let data = match fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error reading '{}': {}", input, e);
            return;
        }
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
        println!(
            "Flags: 0x{:08X} (shader={}, expr={})",
            flags,
            flags & 1 != 0,
            flags & 2 != 0
        );
        println!("StringTable: offset={} size={}", st_off, st_size);
        println!("ConstPool:   offset={} size={}", cp_off, cp_size);
    }

    // Dump hex of first 128 bytes
    let limit = data.len().min(128);
    for (i, chunk) in data[..limit].chunks(16).enumerate() {
        print!("{:08X}  ", i * 16);
        for b in chunk {
            print!("{:02X} ", b);
        }
        println!();
    }
    if data.len() > 128 {
        println!("... ({} more bytes)", data.len() - 128);
    }
}

fn cmd_info(input: &str) {
    let src = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading '{}': {}", input, e);
            return;
        }
    };

    match fcs_core::parser::parse_document(&src) {
        Ok((_, doc)) => {
            println!("Name:      {}", doc.meta.name);
            println!("Artists:   {}", doc.meta.artists.join(", "));
            println!("Charters:  {}", doc.meta.charters.join(", "));
            println!("Offset:    {}ms", doc.meta.offset);
            println!("Version:   {}", doc.meta.version);
            println!("Lines:     {}", doc.judgelines.lines.len());
            let total_notes: usize = doc
                .judgelines
                .lines
                .iter()
                .map(|l| l.notes.instances.len())
                .sum();
            println!("Notes:     {}", total_notes);
            let bpm_count = doc.master_timeline.entries.len();
            println!("BPM stops: {}", bpm_count);
        }
        Err(e) => eprintln!("parse error: {:?}", e),
    }
}

fn cmd_convert(
    input: &str,
    output: Option<&str>,
    format: Option<&str>,
    to_format: Option<&str>,
    pgr_version: i32,
) {
    let src = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading '{}': {}", input, e);
            return;
        }
    };

    // Reverse direction: FCS → target format
    if let Some(to) = to_format {
        let (_, doc) = match fcs_core::parser::parse_document(&src) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("FCS parse error: {:?}", e);
                return;
            }
        };
        let out_str = match to {
            "pgr" => fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json(&doc, pgr_version),
            "rpe" => fcs_converter::from_fcs::rpe_writer::fcs_to_rpe_json(&doc),
            "pec" => fcs_converter::from_fcs::pec_writer::fcs_to_pec(&doc),
            other => {
                eprintln!(
                    "unsupported target format: '{}' (use: pgr, rpe, pec)",
                    other
                );
                return;
            }
        };
        match output {
            Some(out) => {
                fs::write(out, &out_str).unwrap_or_else(|e| eprintln!("write error: {}", e));
                println!("Converted '{}' -> '{}'", input, out);
            }
            None => println!("{}", out_str),
        }
        return;
    }

    // Forward direction: source format → FCS
    let fmt = format.unwrap_or_else(|| detect_format(input, &src));
    let doc = match fmt {
        "pgr" => convert_with(&src, fcs_converter::pgr::parse_pgr, "PGR"),
        "rpe" => convert_with(&src, fcs_converter::rpe::parse_rpe, "RPE"),
        "pec" => convert_with(&src, fcs_converter::pec::parse_pec, "PEC"),
        other => {
            eprintln!("unsupported format: '{}' (supported: pgr, rpe, pec)", other);
            return;
        }
    };
    let fcs_src = format_fcs(&doc);
    match output {
        Some(out) => {
            fs::write(out, &fcs_src).unwrap_or_else(|e| eprintln!("write error: {}", e));
            println!("Converted '{}' -> '{}'", input, out);
        }
        None => println!("{}", fcs_src),
    }
}

fn convert_with(
    src: &str,
    parser: fn(&str) -> Result<fcs_converter::ir::IrChart, String>,
    name: &str,
) -> fcs_core::ast::Document {
    match parser(src) {
        Ok(ir) => fcs_converter::to_fcs::ir_to_fcs(&ir),
        Err(e) => {
            eprintln!("{} parse error: {}", name, e);
            std::process::exit(1);
        }
    }
}

fn detect_format(path: &str, src: &str) -> &'static str {
    let path_lower = path.to_lowercase();
    if path_lower.ends_with(".pec")
        || src
            .lines()
            .next()
            .map(|l| l.trim().parse::<f64>().is_ok())
            .unwrap_or(false)
    {
        return "pec";
    }
    // Try to parse as JSON → check for RPE markers
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(src) {
        if v.get("META").is_some() || v.get("BPMList").is_some() {
            return "rpe";
        }
        if v.get("judgeLineList").is_some() || v.get("formatVersion").is_some() {
            return "pgr";
        }
    }
    // Fallback: extension-based
    "pgr"
}

fn format_fcs(doc: &fcs_core::ast::Document) -> String {
    let mut o = String::new();
    o.push_str("meta {\n");
    o.push_str(&format!("    name: {:?};\n", doc.meta.name));
    let a: Vec<_> = doc
        .meta
        .artists
        .iter()
        .map(|x| format!("{:?}", x))
        .collect();
    o.push_str(&format!("    artists: [{}];\n", a.join(", ")));
    let c: Vec<_> = doc
        .meta
        .charters
        .iter()
        .map(|x| format!("{:?}", x))
        .collect();
    o.push_str(&format!("    charters: [{}];\n", c.join(", ")));
    o.push_str(&format!("    offset: {}ms;\n", doc.meta.offset as i64));
    o.push_str(&format!("    version: {:?};\n", doc.meta.version));
    o.push_str("}\n\nmasterTimeline {\n");
    for e in &doc.master_timeline.entries {
        o.push_str(&format!("    {:.1}b -> {:.1};\n", e.beat, e.bpm));
    }
    o.push_str("}\n\njudgelines {\n");
    for line in &doc.judgelines.lines {
        o.push_str(&format!("    line {} {{\n", line.name));
        o.push_str(&format!("        zOrder: {};\n", line.z_order));
        o.push_str("        bpmTimeline {\n");
        for e in &line.bpm_timeline.entries {
            o.push_str(&format!("            {:.1}b -> {:.1};\n", e.beat, e.bpm));
        }
        o.push_str("        }\n");
        // Motion block
        if let Some(ref motion) = line.motion
            && !motion.layers.is_empty()
        {
            o.push_str("        motion {\n");
            for layer in &motion.layers {
                o.push_str("            layer {\n");
                let props = [
                    ("speed", &layer.speed),
                    ("positionX", &layer.position_x),
                    ("positionY", &layer.position_y),
                    ("rotation", &layer.rotation),
                    ("alpha", &layer.alpha),
                    ("scaleX", &layer.scale_x),
                    ("scaleY", &layer.scale_y),
                ];
                for (name, intervals) in &props {
                    if !intervals.is_empty() {
                        o.push_str(&format!("                {} {{\n", name));
                        for intv in *intervals {
                            let expr_str = fmt_lit_expr(&intv.expression);
                            o.push_str(&format!(
                                "                    [{}b => {}b]: {};\n",
                                intv.start_beat, intv.end_beat, expr_str
                            ));
                        }
                        o.push_str("                }\n");
                    }
                }
                o.push_str("            }\n");
            }
            o.push_str("        }\n");
        }
        if !line.notes.instances.is_empty() {
            o.push_str("        notes {\n");
            for n in &line.notes.instances {
                o.push_str(&format!("            {} {{\n", n.kind.as_str()));
                for (k, v) in &n.properties {
                    o.push_str(&format!("                {}: {};\n", k, fmt_val(v)));
                }
                o.push_str("            }\n");
            }
            o.push_str("        }\n");
        }
        o.push_str("    }\n");
    }
    o.push_str("}\n");
    o
}

fn fmt_val(v: &fcs_core::ast::NotePropertyValue) -> String {
    match v {
        fcs_core::ast::NotePropertyValue::Expr(e) => fmt_lit_expr(e),
        fcs_core::ast::NotePropertyValue::Bool(b) => b.to_string(),
        _ => "?".into(),
    }
}

fn fmt_lit_expr(e: &fcs_core::ast::Expression) -> String {
    match e {
        fcs_core::ast::Expression::Literal(lit) => fmt_literal(lit),
        _ => "?".into(),
    }
}

fn fmt_literal(lit: &fcs_core::ast::Literal) -> String {
    match lit {
        fcs_core::ast::Literal::Float(f) => {
            if f.fract() == 0.0 {
                format!("{:.1}", f)
            } else {
                f.to_string()
            }
        }
        fcs_core::ast::Literal::Integer(n) => n.to_string(),
        fcs_core::ast::Literal::Quantified { value, unit } => {
            format!("{}{}", value, unit_suffix(*unit))
        }
        fcs_core::ast::Literal::Boolean(b) => b.to_string(),
        _ => "?".into(),
    }
}

fn unit_suffix(unit: fcs_core::units::Unit) -> &'static str {
    use fcs_core::units::{AngleUnit, LengthUnit, TimeUnit, Unit};
    match unit {
        Unit::Time(TimeUnit::Millisecond) => "ms",
        Unit::Time(TimeUnit::Second) => "s",
        Unit::Time(TimeUnit::Beat) => "b",
        Unit::Length(LengthUnit::Pixel) => "px",
        Unit::Length(LengthUnit::ViewportWidth) => "vw",
        Unit::Length(LengthUnit::ViewportHeight) => "vh",
        Unit::Angle(AngleUnit::Degree) => "deg",
        Unit::Angle(AngleUnit::Radian) => "rad",
        Unit::Dimensionless => "",
    }
}

#[cfg(test)]
mod tests {
    use fcs_converter::from_fcs::{pec_writer, pgr_writer, rpe_writer};
    use fcs_core::parser;

    const SAMPLE_FCS: &str = r#"meta{name:"T";artists:["A"];charters:["C"];offset:0ms;version:"4.0.0";}masterTimeline{0.0b->120.0;}judgelines{line L{bpmTimeline{0.0b->120.0;}notes{tap{time:4.0b;positionX:0px;}}}}"#;

    #[test]
    fn test_convert_sample_to_pgr() {
        let (_, doc) = parser::parse_document(SAMPLE_FCS).unwrap();
        let json = pgr_writer::fcs_to_pgr_json(&doc, 3);
        assert!(json.contains("\"formatVersion\""));
    }

    #[test]
    fn test_convert_sample_to_rpe() {
        let (_, doc) = parser::parse_document(SAMPLE_FCS).unwrap();
        let json = rpe_writer::fcs_to_rpe_json(&doc);
        assert!(json.contains("\"RPEVersion\""));
    }

    #[test]
    fn test_convert_sample_to_pec() {
        let (_, doc) = parser::parse_document(SAMPLE_FCS).unwrap();
        let pec = pec_writer::fcs_to_pec(&doc);
        assert!(pec.contains("bp "));
    }
}
