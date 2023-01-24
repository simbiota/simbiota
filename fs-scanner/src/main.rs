//! SAFETY: This is a helper tool to test the performance of our implementation and it is
//! not intended to be used in any production system. Most of the unsafe code is necessary to
//! handle CTRL+C and display the results

use clap::Parser;
use simbiota_clientlib::api::detector::{DetectionResult, Detector};
use simbiota_clientlib::client_config::ClientConfig;
use simbiota_clientlib::detector::tlsh_detector::SimpleTLSHDetectorProvider;
use simbiota_clientlib::detector::DetectorProvider;
use simbiota_clientlib::system_database::SystemDatabase;
use std::io::{stdout, Write};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// Specify a custom config file
    #[arg(value_name = "path")]
    pub(crate) path: PathBuf,

    /// Specify a custom config file
    #[arg(short, long, value_name = "config file")]
    pub config: PathBuf,

    /// Verbose output
    #[arg(short, long)]
    pub(crate) verbose: bool,
}

#[derive(Debug)]
struct ScanDirResult {
    errored_paths: Vec<String>,
    detected_paths: Vec<String>,
    dirs_scanned: u64,
    files_scanned: u64,
    bytes_read: u64,
}

impl ScanDirResult {
    pub(crate) fn add_match(&mut self, path: &Path) {
        self.detected_paths.push(path.display().to_string());
    }
}

impl ScanDirResult {
    pub fn new() -> Self {
        Self {
            errored_paths: Vec::new(),
            detected_paths: Vec::new(),
            dirs_scanned: 0,
            files_scanned: 0,
            bytes_read: 0,
        }
    }

    pub fn from_error(path: String) -> Self {
        Self {
            errored_paths: vec![path],
            detected_paths: Vec::new(),
            dirs_scanned: 0,
            files_scanned: 0,
            bytes_read: 0,
        }
    }

    pub fn append(&mut self, mut other: Self) {
        self.errored_paths.append(&mut other.errored_paths);
        self.detected_paths.append(&mut other.detected_paths);
        self.dirs_scanned += other.dirs_scanned;
        self.files_scanned += other.files_scanned;
    }
}

struct RuntimeData {
    start: Option<Instant>,
    result: Option<ScanDirResult>,
}

impl RuntimeData {
    pub const fn new() -> Self {
        Self {
            start: None,
            result: None,
        }
    }
}

static mut RUNTIME_DATA: RuntimeData = RuntimeData::new();

fn print_results() {
    unsafe {
        let elapsed = RUNTIME_DATA.start.unwrap().elapsed();
        let fps =
            RUNTIME_DATA.result.as_mut().unwrap().files_scanned as f64 / elapsed.as_secs() as f64;
        let mbps = RUNTIME_DATA.result.as_mut().unwrap().bytes_read as f64
            / elapsed.as_secs() as f64
            / 1024.0
            / 1024.0;
        println!("Results: {:#?}", RUNTIME_DATA.result.as_ref().unwrap());
        println!(
            "Scanning took {:?}, ({} files/s , {} mbps)",
            elapsed, fps, mbps
        );
        exit(1);
    }
}

fn main() {
    let args = Args::parse();

    let config = ClientConfig::load_from(&args.config, false);
    // Load the database from the filesystem
    let database = Arc::new(Mutex::new(SystemDatabase::load(&config)));

    let mut scanner = Scanner::new(database, config);

    unsafe {
        RUNTIME_DATA.result = Some(ScanDirResult::new());
    }
    print!("Starting scan...");
    ctrlc::set_handler(|| {
        println!("\nScanning interrupted");
        print_results();
    })
    .expect("failed to set ctrl+c handler");
    unsafe {
        RUNTIME_DATA.start = Some(Instant::now());
    }
    if args.path.is_dir() {
        unsafe {
            scanner.scan_dir(&args.path, RUNTIME_DATA.result.as_mut().unwrap());
        }
    } else {
        unsafe {
            scanner.scan_file(&args.path, RUNTIME_DATA.result.as_mut().unwrap());
        }
    }

    println!("\nScanning done");
    print_results();
}

#[allow(dead_code)]
struct Scanner {
    config: ClientConfig,
    detector: Box<dyn Detector>,
    database: Arc<Mutex<SystemDatabase>>,
}

impl Scanner {
    pub fn new(database: Arc<Mutex<SystemDatabase>>, config: ClientConfig) -> Self {
        let detector_provider = SimpleTLSHDetectorProvider::new();
        let detector = detector_provider.get_detector(&config.detector.config, database.clone());
        Self {
            detector,
            database,
            config,
        }
    }

    fn scan_dir(&mut self, dir: &Path, results: &mut ScanDirResult) {
        if !dir.is_dir() {
            panic!("not a directory");
        }
        let Ok(dir_list) = std::fs::read_dir(dir) else {
            results.append(ScanDirResult::from_error(format!("failed to read dir: {}", dir.display())));
            return;
        };
        print!(
            "\rScanning {:<120} ({:<12} files scanned)",
            dir.display(),
            results.files_scanned,
        );
        stdout().flush().unwrap();
        let result = ScanDirResult::new();
        for dir_entry in dir_list {
            match dir_entry {
                Ok(dir) => {
                    let path = dir.path();
                    if path.is_dir() {
                        self.scan_dir(&path, results);
                    } else if path.is_file() {
                        self.scan_file(&path, results);
                    } else {
                        eprintln!("ignored path: {}", path.display());
                    }
                }
                Err(err) => {
                    results.append(ScanDirResult::from_error(format!(
                        "dirlist failed for {}: {err:?}",
                        dir.display()
                    )));
                    return;
                }
            }
        }
        results.dirs_scanned += 1;
        results.append(result);
    }

    fn scan_file(&mut self, file: &Path, results: &mut ScanDirResult) {
        if !file.is_file() {
            panic!("not a file: {}", file.display());
        }

        let contents = std::fs::File::open(file);
        match contents {
            Ok(mut data) => {
                let mut result = ScanDirResult::new();
                let detection_result = self.detector.check_reader(&mut data);
                if let Ok(..) = detection_result {
                    result = ScanDirResult::from_error(format!(
                        "detection error for {}: {:?}",
                        file.display(),
                        detection_result.unwrap_err(),
                    ));
                } else {
                    match detection_result.unwrap() {
                        DetectionResult::Match => {
                            result.add_match(file);
                        }
                        DetectionResult::NoMatch => {}
                    }
                }
                results.bytes_read += data.metadata().unwrap().len();
                results.files_scanned += 1;
                results.append(result);
            }
            Err(error) => {
                results.append(ScanDirResult::from_error(format!(
                    "failed to read file {}: {error:?}",
                    file.display()
                )));
            }
        }
    }
}
