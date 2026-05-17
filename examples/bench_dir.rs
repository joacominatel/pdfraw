//! Benchmark layout-preserving extraction over a PDF or a folder of PDFs.
//!
//! Reports per-file timings and an aggregate summary (min / p50 / mean /
//! p95 / max latency, plus PDFs/sec, pages/sec, chars/sec, MB/sec).
//!
//! Build in release mode so the numbers reflect optimised code:
//!
//! ```text
//! cargo run --release --example bench_dir -- <path>
//! cargo run --release --example bench_dir -- <path> --simple
//! cargo run --release --example bench_dir -- <path> --recursive --csv
//! ```
//!
//! `<path>` can be either a single `.pdf` file or a directory. With
//! `--recursive`, subdirectories are searched too. The first PDF is run
//! once as a warm-up and its timing is discarded; pass `--no-warmup` to
//! disable.

use pdf_extractor::prelude::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct Sample {
    name: String,
    bytes: u64,
    pages: usize,
    chars: usize,
    scanned_pages: usize,
    duration: Duration,
    error: Option<String>,
}

#[derive(Debug, Default)]
struct Options {
    target: Option<PathBuf>,
    simple: bool,
    recursive: bool,
    warmup: bool,
    quiet: bool,
    csv: bool,
}

fn main() {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(code) => std::process::exit(code),
    };
    let Some(target) = opts.target.as_deref() else {
        print_usage();
        std::process::exit(2);
    };

    let pdfs = collect_pdfs(target, opts.recursive);
    if pdfs.is_empty() {
        eprintln!(
            "no PDF files under {} (use --recursive to search subdirectories)",
            target.display()
        );
        std::process::exit(1);
    }

    let mode_label = if opts.simple {
        "extract_text"
    } else {
        "extract_text_layout"
    };

    if !opts.csv {
        println!(
            "{} PDF(s) under {} — mode: {}{}\n",
            pdfs.len(),
            target.display(),
            mode_label,
            if opts.warmup { " (with warm-up)" } else { "" },
        );
    }

    if opts.warmup {
        // Warm the file cache and process-local state by running once over
        // the first PDF; its timing is discarded.
        let text_opts = TextOptions::pdfplumber_defaults();
        let _ = run_one(&pdfs[0], opts.simple, &text_opts);
    }

    let text_opts = TextOptions::pdfplumber_defaults();
    let mut samples = Vec::with_capacity(pdfs.len());
    let total_start = Instant::now();

    for path in &pdfs {
        let s = run_one(path, opts.simple, &text_opts);
        if !opts.quiet && !opts.csv {
            print_row(&s);
        }
        if opts.csv {
            print_csv_row(&s);
        }
        samples.push(s);
    }

    let total_elapsed = total_start.elapsed();

    if !opts.csv {
        if !opts.quiet {
            println!();
        }
        print_summary(&samples, total_elapsed);
    }
}

fn parse_args() -> std::result::Result<Options, i32> {
    let mut opts = Options {
        warmup: true,
        ..Options::default()
    };
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                return Err(0);
            }
            "--simple" => opts.simple = true,
            "--recursive" | "-r" => opts.recursive = true,
            "--no-warmup" => opts.warmup = false,
            "--quiet" | "-q" => opts.quiet = true,
            "--csv" => opts.csv = true,
            other if other.starts_with("--") => {
                eprintln!("unknown flag: {other}");
                print_usage();
                return Err(2);
            }
            other => {
                if opts.target.is_some() {
                    eprintln!("multiple paths given; only one is supported");
                    return Err(2);
                }
                opts.target = Some(PathBuf::from(other));
            }
        }
    }
    Ok(opts)
}

fn print_usage() {
    eprintln!(
        "Usage: bench_dir <pdf-or-dir> [OPTIONS]\n\
         \n\
         Arguments:\n  \
           <pdf-or-dir>  A single .pdf file or a folder containing .pdf files.\n\
         \n\
         Options:\n  \
           --simple      Use Page::extract_text instead of layout reconstruction.\n  \
           -r, --recursive   Descend into subdirectories.\n  \
           --no-warmup   Skip the warm-up pass (the first PDF, timing discarded).\n  \
           -q, --quiet   Suppress per-file rows; print the summary only.\n  \
           --csv         Print machine-readable CSV instead of the summary.\n  \
           -h, --help    Show this help.\n"
    );
}

fn collect_pdfs(target: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if target.is_file() {
        if has_pdf_extension(target) {
            out.push(target.to_path_buf());
        }
        return out;
    }
    if target.is_dir() {
        walk_dir(target, recursive, &mut out);
    }
    out.sort();
    out
}

fn walk_dir(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            if recursive {
                walk_dir(&path, true, out);
            }
        } else if has_pdf_extension(&path) {
            out.push(path);
        }
    }
}

fn has_pdf_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

fn run_one(path: &Path, simple: bool, text_opts: &TextOptions) -> Sample {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string();
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let start = Instant::now();
    let result = run_extraction(path, simple, text_opts);
    let duration = start.elapsed();

    match result {
        Ok((pages, chars, scanned_pages)) => Sample {
            name,
            bytes,
            pages,
            chars,
            scanned_pages,
            duration,
            error: None,
        },
        Err(e) => Sample {
            name,
            bytes,
            pages: 0,
            chars: 0,
            scanned_pages: 0,
            duration,
            error: Some(format!("{e}")),
        },
    }
}

fn run_extraction(
    path: &Path,
    simple: bool,
    text_opts: &TextOptions,
) -> Result<(usize, usize, usize)> {
    let doc = Document::open(path)?;
    let mut chars_total = 0usize;
    let mut scanned = 0usize;
    let pages = doc.num_pages();
    for page in doc.pages() {
        let page = page?;
        if page.is_scanned()? {
            scanned += 1;
            continue;
        }
        let cs = page.chars()?;
        chars_total += cs.len();
        if simple {
            let _ = page.extract_text()?;
        } else {
            let _ = page.extract_text_layout(text_opts)?;
        }
    }
    Ok((pages, chars_total, scanned))
}

fn print_row(s: &Sample) {
    let ms = s.duration.as_secs_f64() * 1000.0;
    if let Some(err) = &s.error {
        println!(
            "  x {:45} {:>7.1} ms   ERROR: {}",
            truncate(&s.name, 45),
            ms,
            err
        );
        return;
    }
    let kb = s.bytes as f64 / 1024.0;
    let ms_per_page = if s.pages > 0 {
        ms / s.pages as f64
    } else {
        0.0
    };
    let scan_tag = if s.scanned_pages > 0 {
        format!(" [{} scan]", s.scanned_pages)
    } else {
        String::new()
    };
    println!(
        "  + {:45} {:>7.1} KB {:>3} pg {:>6} ch {:>7.1} ms  ({:>5.2} ms/pg){}",
        truncate(&s.name, 45),
        kb,
        s.pages,
        s.chars,
        ms,
        ms_per_page,
        scan_tag,
    );
}

fn print_csv_row(s: &Sample) {
    // Header is printed once by the caller when csv mode begins. We print
    // one row per sample here; downstream tools can pipe through `sort`,
    // `awk`, etc.
    static HEADER_PRINTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    if HEADER_PRINTED.set(()).is_ok() {
        println!("name,bytes,pages,chars,scanned_pages,duration_ms,error");
    }
    let ms = s.duration.as_secs_f64() * 1000.0;
    let err = s
        .error
        .as_deref()
        .map(|e| e.replace(',', ";"))
        .unwrap_or_default();
    println!(
        "{},{},{},{},{},{:.3},{}",
        csv_field(&s.name),
        s.bytes,
        s.pages,
        s.chars,
        s.scanned_pages,
        ms,
        err
    );
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn print_summary(samples: &[Sample], total_elapsed: Duration) {
    let ok: Vec<&Sample> = samples.iter().filter(|s| s.error.is_none()).collect();
    let failed = samples.len() - ok.len();
    let scanned_files = ok.iter().filter(|s| s.scanned_pages > 0).count();
    let total_pages: usize = ok.iter().map(|s| s.pages).sum();
    let total_chars: usize = ok.iter().map(|s| s.chars).sum();
    let total_bytes: u64 = ok.iter().map(|s| s.bytes).sum();

    let mut durations_ms: Vec<f64> = ok
        .iter()
        .map(|s| s.duration.as_secs_f64() * 1000.0)
        .collect();
    durations_ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mean = mean(&durations_ms);
    let p50 = percentile(&durations_ms, 50.0);
    let p95 = percentile(&durations_ms, 95.0);
    let min = durations_ms.first().copied().unwrap_or(0.0);
    let max = durations_ms.last().copied().unwrap_or(0.0);

    let total_secs = total_elapsed.as_secs_f64();
    let pdfs_per_sec = ok.len() as f64 / total_secs.max(1e-9);
    let pages_per_sec = total_pages as f64 / total_secs.max(1e-9);
    let chars_per_sec = total_chars as f64 / total_secs.max(1e-9);
    let mb_per_sec = (total_bytes as f64 / 1024.0 / 1024.0) / total_secs.max(1e-9);

    println!("--- Summary -------------------------------------");
    println!(
        "Files:        {} ok, {} failed, {} with scanned pages",
        ok.len(),
        failed,
        scanned_files
    );
    println!("Pages:        {}", total_pages);
    println!("Chars:        {}", total_chars);
    println!("Total bytes:  {:.1} KB", total_bytes as f64 / 1024.0);
    println!("Total time:   {:.2} s", total_secs);
    println!();
    println!("Per-PDF latency (ms):");
    println!("  min:   {:>7.2}", min);
    println!("  p50:   {:>7.2}", p50);
    println!("  mean:  {:>7.2}", mean);
    println!("  p95:   {:>7.2}", p95);
    println!("  max:   {:>7.2}", max);
    println!();
    println!("Throughput:");
    println!("  {:>7.1} PDFs/sec", pdfs_per_sec);
    println!("  {:>7.1} pages/sec", pages_per_sec);
    println!("  {:>7.0} chars/sec", chars_per_sec);
    println!("  {:>7.2} MB/sec", mb_per_sec);
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (p / 100.0) * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let frac = rank - lo as f64;
        sorted[lo] * (1.0 - frac) + sorted[hi] * frac
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max - 1).collect();
        t.push('.');
        t
    }
}
