use clap::AppSettings::DeriveDisplayOrder;
use clap::Parser;
use extract_from_bam::Data;
use log::{error, info};
use std::path::PathBuf;

pub mod calculations;
pub mod extract_from_bam;
pub mod feather;
pub mod file_info;
pub mod histograms;
pub mod karyotype;
pub mod phased;
pub mod splicing;
pub mod utils;

// The arguments end up in the Cli struct
#[derive(Parser, Debug)]
#[structopt(global_settings=&[DeriveDisplayOrder])]
#[clap(author, version, about="Tool to extract QC metrics from cram or bam", long_about = None)]
struct Cli {
    /// cram or bam file to check
    #[clap(value_parser, validator=is_file)]
    input: String,

    /// Number of parallel decompression threads to use
    #[clap(short, long, value_parser, default_value_t = 4)]
    threads: usize,

    /// reference for decompressing bam/cram
    #[clap(long, value_parser)]
    reference: Option<String>,

    /// Minimal length of read to be considered
    #[clap(short, long, value_parser, default_value_t = 0)]
    min_read_len: usize,

    /// If histograms have to be generated
    #[clap(long, value_parser)]
    hist: bool,

    /// If a checksum has to be calculated
    #[clap(long, value_parser)]
    checksum: bool,

    /// Write data to a feather format
    #[clap(long, value_parser)]
    arrow: Option<String>,

    /// Provide normalized number of reads per chromosome
    #[clap(long, value_parser)]
    karyotype: bool,

    /// Calculate metrics for phased reads
    #[clap(long, value_parser)]
    phased: bool,

    /// Provide metrics for spliced data
    #[clap(long, value_parser)]
    spliced: bool,
}

pub fn is_file(pathname: &str) -> Result<(), String> {
    if pathname == "-" {
        return Ok(());
    }
    let path = PathBuf::from(pathname);
    if path.is_file() {
        Ok(())
    } else {
        Err(format!("Input file {} is invalid", path.display()))
    }
}

fn main() {
    env_logger::init();
    let args = Cli::parse();
    info!("Collected arguments");
    let metrics = extract_from_bam::extract(
        &args.input,
        args.threads,
        args.reference,
        args.min_read_len,
        args.arrow,
        args.karyotype,
        args.phased,
        args.spliced,
    );

    metrics_from_bam(
        metrics,
        args.input,
        args.hist,
        args.checksum,
        args.karyotype,
        args.phased,
        args.spliced,
    );
    info!("Finished");
}

fn metrics_from_bam(
    metrics: Data,
    bam: String,
    hist: bool,
    checksum: bool,
    karyotype: bool,
    phased: bool,
    spliced: bool,
) {
    let bam = file_info::BamFile { path: bam };
    println!("File name\t{}", bam.file_name());

    generate_main_output(
        metrics.lengths.as_ref().unwrap(),
        metrics.identities.as_ref().unwrap(),
        utils::get_genome_size(&bam.path),
    );

    println!("Path\t{}", bam);
    println!("Creation time\t{}", bam.file_time());
    if checksum {
        println!("Checksum\t{}", bam.checksum());
    }

    let phaseblocks = if phased {
        Some(phased::phase_metrics(
            metrics.tids.as_ref().unwrap(),
            metrics.starts.unwrap(),
            metrics.ends.unwrap(),
            metrics.phasesets.as_ref().unwrap(),
        ))
    } else {
        None
    };
    if karyotype {
        karyotype::make_karyotype(metrics.tids.as_ref().unwrap(), bam.to_string());
    }
    if spliced {
        splicing::splice_metrics(metrics.exons.unwrap());
    }
    if hist {
        histograms::make_histogram_lengths(metrics.lengths.as_ref().unwrap());
        histograms::make_histogram_identities(metrics.identities.as_ref().unwrap());
        if phased {
            histograms::make_histogram_phaseblocks(&phaseblocks.unwrap())
        }
    }
}

fn generate_main_output(lengths: &Vec<u64>, identities: &[f64], genome_size: u64) {
    let num_reads = lengths.len();
    if num_reads < 2 {
        error!("Not enough reads to calculate metrics!");
        panic!();
    }
    let data_yield: u64 = lengths.iter().sum::<u64>();
    println!("Number of reads\t{num_reads}");
    println!("Yield [Gb]\t{:.2}", data_yield as f64 / 1e9);
    println!(
        "Mean coverage\t{:.2}",
        data_yield as f64 / genome_size as f64
    );
    println!("N50\t{}", calculations::get_n50(lengths, data_yield));
    println!("Median length\t{:.2}", calculations::median_length(lengths));
    println!("Mean length\t{:.2}", data_yield / num_reads as u64);
    println!("Median identity\t{:.2}", calculations::median(identities));
    println!(
        "Mean identity\t{:.2}",
        identities.iter().sum::<f64>() / (num_reads as f64)
    );
}

#[cfg(test)]
#[ctor::ctor]
fn init() {
    env_logger::init();
}

#[test]
fn verify_app() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
}

#[test]
fn extract() {
    let metrics = extract_from_bam::extract(
        &"test-data/small-test-phased.bam".to_string(),
        8,
        None,
        0,
        Some("test.feather".to_string()),
        true,
        true,
        false,
    );
    metrics_from_bam(
        metrics,
        "test-data/small-test-phased.bam".to_string(),
        true,
        true,
        true,
        true,
        false,
    )
}
