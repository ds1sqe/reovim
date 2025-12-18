//! Performance report generator for reovim benchmarks
//!
//! Reads Criterion benchmark results and generates/updates PERF-{version}.md files.

use {
    chrono::{DateTime, Utc},
    clap::{Parser, Subcommand},
    serde::{Deserialize, Serialize},
    std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
        process::Command,
    },
    walkdir::WalkDir,
};

/// Performance report generator for reovim benchmarks
#[derive(Parser)]
#[command(name = "perf-report")]
#[command(about = "Generate and manage performance benchmark reports")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to the criterion output directory
    #[arg(long, default_value = "target/criterion")]
    criterion_dir: PathBuf,

    /// Path to the perf reports directory
    #[arg(long, default_value = "perf")]
    perf_dir: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    /// Run benchmarks, clear old data, and generate report (all-in-one)
    Bench {
        /// Version string (e.g., "0.4.5")
        #[arg(long, short)]
        version: String,
        /// Skip clearing old criterion data
        #[arg(long)]
        no_clean: bool,
    },
    /// Update or create a performance report for a version
    Update {
        /// Version string (e.g., "0.3.0")
        #[arg(long, short)]
        version: String,
    },
    /// Check current results against baselines
    Check {
        /// Fail if regressions exceed max threshold
        #[arg(long)]
        fail_on_regression: bool,
    },
    /// Compare two versions
    Compare {
        /// Previous version
        old: String,
        /// New version
        new: String,
    },
    /// List all benchmark results from criterion
    List,
}

/// Criterion estimates.json structure
#[derive(Debug, Deserialize)]
struct CriterionEstimates {
    mean: Estimate,
    median: Estimate,
    std_dev: Estimate,
    #[serde(default)]
    #[allow(dead_code)]
    slope: Option<Estimate>,
}

#[derive(Debug, Deserialize)]
struct Estimate {
    point_estimate: f64,
    #[allow(dead_code)]
    standard_error: f64,
    confidence_interval: ConfidenceInterval,
}

#[derive(Debug, Deserialize)]
struct ConfidenceInterval {
    #[allow(dead_code)]
    confidence_level: f64,
    lower_bound: f64,
    upper_bound: f64,
}

/// Benchmark result
#[derive(Debug, Clone, Serialize)]
struct BenchResult {
    mean: f64,
    median: f64,
    std_dev: f64,
    ci_lower: f64,
    ci_upper: f64,
    unit: String,
}

impl BenchResult {
    fn from_estimates(estimates: &CriterionEstimates) -> Self {
        // Criterion reports in nanoseconds
        let mean_ns = estimates.mean.point_estimate;
        let median_ns = estimates.median.point_estimate;
        let std_dev_ns = estimates.std_dev.point_estimate;

        // Determine appropriate unit and convert
        let (mean, median, std_dev, ci_lower, ci_upper, unit) = if mean_ns >= 1_000_000_000.0 {
            (
                mean_ns / 1_000_000_000.0,
                median_ns / 1_000_000_000.0,
                std_dev_ns / 1_000_000_000.0,
                estimates.mean.confidence_interval.lower_bound / 1_000_000_000.0,
                estimates.mean.confidence_interval.upper_bound / 1_000_000_000.0,
                "s".to_string(),
            )
        } else if mean_ns >= 1_000_000.0 {
            (
                mean_ns / 1_000_000.0,
                median_ns / 1_000_000.0,
                std_dev_ns / 1_000_000.0,
                estimates.mean.confidence_interval.lower_bound / 1_000_000.0,
                estimates.mean.confidence_interval.upper_bound / 1_000_000.0,
                "ms".to_string(),
            )
        } else if mean_ns >= 1_000.0 {
            (
                mean_ns / 1_000.0,
                median_ns / 1_000.0,
                std_dev_ns / 1_000.0,
                estimates.mean.confidence_interval.lower_bound / 1_000.0,
                estimates.mean.confidence_interval.upper_bound / 1_000.0,
                "µs".to_string(),
            )
        } else {
            (
                mean_ns,
                median_ns,
                std_dev_ns,
                estimates.mean.confidence_interval.lower_bound,
                estimates.mean.confidence_interval.upper_bound,
                "ns".to_string(),
            )
        };

        Self {
            mean,
            median,
            std_dev,
            ci_lower,
            ci_upper,
            unit,
        }
    }
}

/// Metadata for the report
#[derive(Debug, Serialize)]
struct Metadata {
    version: String,
    commit: String,
    date: String,
    rust_version: String,
    os: String,
}

impl Metadata {
    fn collect(version: &str) -> Self {
        let commit = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let rust_version = Command::new("rustc")
            .args(["--version"])
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("unknown")
                    .to_string()
            })
            .unwrap_or_else(|_| "unknown".to_string());

        let os = Command::new("uname")
            .args(["-sr"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let date: DateTime<Utc> = Utc::now();

        Self {
            version: version.to_string(),
            commit,
            date: date.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            rust_version,
            os,
        }
    }
}

/// Read all benchmark results from criterion directory
fn read_criterion_results(criterion_dir: &Path) -> BTreeMap<String, BenchResult> {
    let mut results = BTreeMap::new();

    for entry in WalkDir::new(criterion_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.ends_with("new/estimates.json")
            && let Ok(content) = fs::read_to_string(path)
            && let Ok(estimates) = serde_json::from_str::<CriterionEstimates>(&content)
        {
            // Extract benchmark name from path
            // e.g., target/criterion/window_render/buffer_lines/10/new/estimates.json
            // becomes: window_render/buffer_lines/10
            let bench_path = path
                .strip_prefix(criterion_dir)
                .unwrap_or(path)
                .parent() // remove "estimates.json"
                .and_then(|p| p.parent()) // remove "new"
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            if !bench_path.is_empty() && bench_path != "report" {
                results.insert(bench_path, BenchResult::from_estimates(&estimates));
            }
        }
    }

    results
}

/// Generate the markdown report
fn generate_report(
    version: &str,
    metadata: &Metadata,
    results: &BTreeMap<String, BenchResult>,
) -> String {
    let mut report = String::new();

    // Header
    report.push_str(&format!("# Performance Benchmarks v{version}\n\n"));
    report.push_str(&format!(
        "> Last updated: {} | Commit: {} | Rust: {}\n\n",
        metadata.date, metadata.commit, metadata.rust_version
    ));

    // Metadata TOML block
    report.push_str("## Metadata\n\n```toml\n[metadata]\n");
    report.push_str(&format!("version = \"{}\"\n", metadata.version));
    report.push_str(&format!("commit = \"{}\"\n", metadata.commit));
    report.push_str(&format!("date = \"{}\"\n", metadata.date));
    report.push_str(&format!("rust_version = \"{}\"\n", metadata.rust_version));
    report.push_str(&format!("os = \"{}\"\n", metadata.os));
    report.push_str("```\n\n");

    // Group results by category
    let mut categories: BTreeMap<String, Vec<(&String, &BenchResult)>> = BTreeMap::new();
    for (name, result) in results {
        let category = name.split('/').next().unwrap_or("other").to_string();
        categories.entry(category).or_default().push((name, result));
    }

    // Summary table
    report.push_str("## Summary\n\n");
    report.push_str("| Category | Benchmarks | Avg Time |\n");
    report.push_str("|----------|------------|----------|\n");
    for (category, benches) in &categories {
        let avg: f64 = benches.iter().map(|(_, r)| r.mean).sum::<f64>() / benches.len() as f64;
        let unit = benches
            .first()
            .map(|(_, r)| r.unit.as_str())
            .unwrap_or("µs");
        report.push_str(&format!("| {} | {} | {:.2} {} |\n", category, benches.len(), avg, unit));
    }
    report.push('\n');

    // Results TOML block
    report.push_str("## Results\n\n```toml\n");
    for (category, benches) in &categories {
        report.push_str(&format!("[results.{}]\n", category.replace('-', "_")));
        for (name, result) in benches {
            let short_name = name
                .strip_prefix(&format!("{category}/"))
                .unwrap_or(name)
                .replace('/', "_");
            report.push_str(&format!(
                "{} = {{ mean = {:.2}, median = {:.2}, std_dev = {:.2}, unit = \"{}\" }}\n",
                short_name, result.mean, result.median, result.std_dev, result.unit
            ));
        }
        report.push('\n');
    }
    report.push_str("```\n\n");

    // Detailed results table
    report.push_str("## Detailed Results\n\n");
    report.push_str("| Benchmark | Mean | Median | Std Dev | CI (95%) |\n");
    report.push_str("|-----------|------|--------|---------|----------|\n");
    for (name, result) in results {
        report.push_str(&format!(
            "| {} | {:.2} {} | {:.2} {} | {:.2} {} | [{:.2}, {:.2}] {} |\n",
            name,
            result.mean,
            result.unit,
            result.median,
            result.unit,
            result.std_dev,
            result.unit,
            result.ci_lower,
            result.ci_upper,
            result.unit
        ));
    }

    report
}

fn cmd_bench(cli: &Cli, version: &str, no_clean: bool) -> Result<(), Box<dyn std::error::Error>> {
    // Step 1: Clear old criterion data
    if !no_clean && cli.criterion_dir.exists() {
        println!("Clearing old benchmark data from {:?}...", cli.criterion_dir);
        fs::remove_dir_all(&cli.criterion_dir)?;
    }

    // Step 2: Run benchmarks
    println!("Running benchmarks...\n");
    let status = Command::new("cargo")
        .args(["bench", "-p", "reovim-core"])
        .status()?;

    if !status.success() {
        eprintln!("Benchmark failed");
        std::process::exit(1);
    }

    // Step 3: Generate report
    println!();
    cmd_update(cli, version)?;

    println!("\n✓ Benchmarks complete! Report: perf/PERF-{}.md", version);
    Ok(())
}

fn cmd_update(cli: &Cli, version: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Reading benchmark results from {:?}...", cli.criterion_dir);
    let results = read_criterion_results(&cli.criterion_dir);

    if results.is_empty() {
        eprintln!("No benchmark results found. Run `cargo bench -p reovim-core` first.");
        std::process::exit(1);
    }

    println!("Found {} benchmark results", results.len());

    let metadata = Metadata::collect(version);
    let report = generate_report(version, &metadata, &results);

    // Ensure perf directory exists
    fs::create_dir_all(&cli.perf_dir)?;

    let output_path = cli.perf_dir.join(format!("PERF-{version}.md"));
    fs::write(&output_path, report)?;

    println!("Report written to {:?}", output_path);
    Ok(())
}

fn cmd_list(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let results = read_criterion_results(&cli.criterion_dir);

    if results.is_empty() {
        println!("No benchmark results found in {:?}", cli.criterion_dir);
        return Ok(());
    }

    println!("Benchmark Results ({} total):\n", results.len());
    for (name, result) in &results {
        println!("  {}: {:.2} {} (±{:.2})", name, result.mean, result.unit, result.std_dev);
    }
    Ok(())
}

fn cmd_check(cli: &Cli, fail_on_regression: bool) -> Result<(), Box<dyn std::error::Error>> {
    let results = read_criterion_results(&cli.criterion_dir);

    if results.is_empty() {
        eprintln!("No benchmark results found.");
        std::process::exit(1);
    }

    // TODO: Load baselines from existing PERF file and compare
    // For now, just print results
    println!("Current benchmark results:");
    for (name, result) in &results {
        println!("  {}: {:.2} {} (±{:.2})", name, result.mean, result.unit, result.std_dev);
    }

    if fail_on_regression {
        println!("\nNo baseline to compare against yet.");
    }

    Ok(())
}

fn cmd_compare(
    cli: &Cli,
    old_version: &str,
    new_version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let old_path = cli.perf_dir.join(format!("PERF-{old_version}.md"));
    let new_path = cli.perf_dir.join(format!("PERF-{new_version}.md"));

    if !old_path.exists() {
        eprintln!("Old version report not found: {:?}", old_path);
        std::process::exit(1);
    }
    if !new_path.exists() {
        eprintln!("New version report not found: {:?}", new_path);
        std::process::exit(1);
    }

    println!("Comparing {} vs {}", old_version, new_version);
    println!("Old: {:?}", old_path);
    println!("New: {:?}", new_path);

    // TODO: Parse TOML from both files and compare
    println!("\nComparison not yet implemented. Use `diff` for now:");
    println!("  diff {:?} {:?}", old_path, new_path);

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Bench { version, no_clean } => cmd_bench(&cli, version, *no_clean)?,
        Commands::Update { version } => cmd_update(&cli, version)?,
        Commands::List => cmd_list(&cli)?,
        Commands::Check { fail_on_regression } => cmd_check(&cli, *fail_on_regression)?,
        Commands::Compare { old, new } => cmd_compare(&cli, old, new)?,
    }

    Ok(())
}
