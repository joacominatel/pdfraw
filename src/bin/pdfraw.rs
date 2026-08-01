//! Command-line front end: extract layout-preserving text from a PDF.
//!
//! ```text
//! pdfraw <file.pdf>             write the text to stdout
//! pdfraw <file.pdf> -o out.txt  write the text to a file
//! pdfraw --version              print the version and exit
//! ```
//!
//! Pages that carry no text layer are skipped with a note on stderr, so a
//! scanned page never silently disappears from the output.

// Deliberately not `use pdfraw::prelude::*`: the prelude re-exports a
// single-parameter `Result` alias that would shadow `std::result::Result`.
use pdfraw::{Document, TextOptions};
use std::io::{self, Write};
use std::process::ExitCode;

const USAGE: &str = "\
usage: pdfraw <file.pdf> [-o <output.txt>]
       pdfraw --version

Extracts layout-preserving text from a PDF. Writes to stdout unless -o is
given. Pages with no text layer are skipped and reported on stderr.";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        // Asking for help or the version is not a failure.
        Err(e) if e.is_request => {
            println!("{}", e.message);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("pdfraw: {}", e.message);
            ExitCode::FAILURE
        }
    }
}

/// A message for the user, and whether it was asked for.
///
/// `--help` and `--version` are not failures: they print to stdout and exit
/// 0. Threading them back through the error path keeps argument handling in
/// one place, at the cost of this flag.
struct Failure {
    message: String,
    /// The user asked for this output, so it is a success.
    is_request: bool,
}

impl<T: Into<String>> From<T> for Failure {
    fn from(message: T) -> Self {
        Self {
            message: message.into(),
            is_request: false,
        }
    }
}

/// Parsed command line.
struct Args {
    input: String,
    output: Option<String>,
}

fn parse_args() -> Result<Args, Failure> {
    let mut input = None;
    let mut output = None;
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                return Err(Failure {
                    message: USAGE.to_string(),
                    is_request: true,
                });
            }
            // Read from the manifest, so the binary cannot claim a version
            // it was not built from. A stale install is otherwise invisible:
            // 0.1.0 wrote `<input>.txt` beside the input and said nothing
            // about which build produced it.
            "-V" | "--version" => {
                return Err(Failure {
                    message: format!("pdfraw {}", env!("CARGO_PKG_VERSION")),
                    is_request: true,
                });
            }
            "-o" | "--output" => {
                output = Some(
                    args.next()
                        .ok_or_else(|| Failure::from("-o needs a path"))?,
                );
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other}\n\n{USAGE}").into());
            }
            other => {
                if input.replace(other.to_string()).is_some() {
                    return Err(format!("expected one input file\n\n{USAGE}").into());
                }
            }
        }
    }

    let input = input.ok_or_else(|| Failure::from(format!("no input file\n\n{USAGE}")))?;
    Ok(Args { input, output })
}

fn run() -> Result<(), Failure> {
    let args = parse_args()?;

    let doc = Document::open(&args.input).map_err(|e| format!("{}: {e}", args.input))?;
    let opts = TextOptions::pdfplumber_defaults();

    let mut out = String::new();
    let mut skipped = Vec::new();

    for page in doc.pages() {
        let page = page.map_err(|e| e.to_string())?;
        if page.is_scanned().map_err(|e| e.to_string())? {
            skipped.push(page.index());
            continue;
        }
        out.push_str(&page.extract_text_layout(&opts).map_err(|e| e.to_string())?);
        out.push('\n');
    }

    match &args.output {
        Some(path) => std::fs::write(path, &out).map_err(|e| format!("{path}: {e}"))?,
        // A broken pipe (`pdfraw x.pdf | head`) is a normal way for this to
        // end, not an error worth a message and a non-zero exit.
        None => match io::stdout().write_all(out.as_bytes()) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => return Ok(()),
            Err(e) => return Err(e.to_string().into()),
        },
    }

    if !skipped.is_empty() {
        let list: Vec<String> = skipped.iter().map(|i| i.to_string()).collect();
        eprintln!(
            "pdfraw: skipped {} page(s) with no text layer (index {}) — these need OCR",
            skipped.len(),
            list.join(", ")
        );
    }

    Ok(())
}
