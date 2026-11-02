//! Native command line: CV JSON file in, Europass XML CV out.
//!
//! ```text
//! europass-xml-cli <input.json> [-o <out.xml>] [--format legacy|candidate] [--options '<json>']
//! ```
//! Exit codes: 0 ok, 1 export error (`[code] message` on stderr), 2 usage or I/O error.
//! Warnings are printed to stderr as `warning: [code] path: message`.
use europass_xml::{export::export_json, options::Options};
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("usage: europass-xml-cli <input.json> [-o <out.xml>] [--format legacy|candidate] [--options '<json>']");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut input: Option<String> = None;
    let mut output: Option<String> = None;
    let mut options: Option<String> = None;
    let mut format: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-o" | "--output" => output = args.next(),
            "--options" => options = args.next(),
            "--format" => format = args.next(),
            "-h" | "--help" => return usage(),
            _ if input.is_none() && !a.starts_with('-') => input = Some(a),
            _ => return usage(),
        }
    }
    let Some(input) = input else {
        return usage();
    };
    let json = match std::fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {input}: {e}");
            return ExitCode::from(2);
        }
    };
    let opts = match Options::from_json(options.as_deref()) {
        Ok(mut o) => {
            match format.as_deref() {
                None => {}
                Some("legacy") => o.format = europass_xml::options::Format::Legacy,
                Some("candidate") => o.format = europass_xml::options::Format::Candidate,
                Some(_) => return usage(),
            }
            o
        }
        Err(e) => {
            eprintln!("error: [{}] {e}", e.code());
            return ExitCode::from(1);
        }
    };
    let export = match export_json(&json, &opts) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("error: [{}] {e}", e.code());
            return ExitCode::from(1);
        }
    };
    for w in &export.warnings {
        eprintln!("warning: [{}] {}: {}", w.code, w.path, w.message);
    }
    match output {
        Some(path) => {
            if let Err(e) = std::fs::write(&path, &export.xml) {
                eprintln!("error: cannot write {path}: {e}");
                return ExitCode::from(2);
            }
        }
        None => print!("{}", export.xml),
    }
    ExitCode::SUCCESS
}
