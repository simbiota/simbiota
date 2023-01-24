//! Traits for hash-based detectir implementations

use crate::api::detector::{DetectionResult, Detector};
use log::debug;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Read;
use std::marker::PhantomData;
use std::time::Instant;

/// The `ComparableHash` trait allows for hashes that can compare themselves to
/// each other and result in some kind of metric
///
/// The resulting metric can be anything that can be used for deciding whether a given hash
/// matches something in the database somehow (e.g TLSH distance < 40).
pub trait ComparableHash {
    type ResultType: Copy;
    /// Calculate the difference between two hashes.
    fn diff(&self, other: &Self) -> Self::ResultType;
    /// Get the raw digest bytes
    fn get_digest(&self) -> Box<[u8]>;
    /// Get the hex representation of the digest
    fn get_digest_hex(&self) -> String;
    fn color(&self) -> u8;
}

/// The `HashAlg` trait allows for hashing algorithms that results in a given
/// `ComparableHash`.
///
/// These are used for malware detectors that calculate the hash of a sample
/// and check it against a database of the same type of `ComparableHash`-es.
pub trait HashAlg<H>
where
    H: ComparableHash,
{
    /// Create a new HashAlg implementation instance.
    fn new() -> Self;
    /// Update the internal state with the given byte slice.
    fn update(&mut self, data_buffer: &[u8]);
    /// Finalize the hash calculation, no `update()` can be called after this.
    fn finalize(&mut self);
    /// Returns the [`ComparableHash`] instance.
    fn get_hash(&self) -> Option<H>;
}

#[derive(Debug)]
pub struct HashError {
    pub message: String,
}
impl Display for HashError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "HashError: {}", self.message)
    }
}
impl Error for HashError {}

/// The `HashDatabase` trait allows for retrieving a list of specific hashes from a database.
///
/// Implementors support providing a list of hashes to the detection engine for comparisons.
///
/// Please note that `get_hashes()` are called for _every_ detection request, the performance of
/// this method directly influences the performance of the antivirus detection system.
pub trait HashDatabase<H>
where
    H: ComparableHash,
{
    /// Return a slice of `T` typed ComparableHash that will be used for malware
    /// detection.
    fn get_hashes(&mut self) -> &[H];
}

/// The `HashBasedDetector` trait enables the customization of the hash against database comparison of the [`Detector`].
///
/// Implementors can do whatever they want with the database and the hash that results in a [`DetectionResult`]. An example
/// implementation that implements the diff-against-all-less-than-40 from the original SIMBIoTA paper can be seen in [`CompareAgainstAllDetector`].
pub trait HashBasedDetector<'a, H>
where
    H: ComparableHash,
{
    fn do_detect(&mut self, hash: &H) -> Result<DetectionResult, Box<dyn Error>>;
}

/// Used for buffered reading in [`AbstractHashBasedDetector`], specifies
/// the read buffer size
const READ_BUFFER_SIZE: usize = 1024;

/// A [`Detector`] implementation to be used with various hashes that can be calculated from the input data
pub struct AbstractHashBasedDetector<'a, A, H>
where
    H: ComparableHash,
    A: HashAlg<H>,
{
    detector_impl: Box<dyn HashBasedDetector<'a, H>>,
    _phantom: PhantomData<A>,
}

impl<'a, A, H> AbstractHashBasedDetector<'a, A, H>
where
    H: ComparableHash,
    A: HashAlg<H>,
{
    pub fn new(detector_impl: Box<dyn HashBasedDetector<'a, H>>) -> Self {
        Self {
            detector_impl,
            _phantom: PhantomData,
        }
    }

    /// Called by the detector trait implmentations wi
    fn do_detect(&mut self, hash: H) -> Result<DetectionResult, Box<dyn Error>> {
        self.detector_impl.do_detect(&hash)
    }
}

impl<'a, A, H> Detector for AbstractHashBasedDetector<'a, A, H>
where
    H: ComparableHash,
    A: HashAlg<H>,
{
    fn check_bytes(&mut self, input_bytes: &[u8]) -> Result<DetectionResult, Box<dyn Error>> {
        let mut tlsh = A::new();
        tlsh.update(input_bytes);
        tlsh.finalize();
        let Some(hash) = tlsh.get_hash() else {
            return Err(Box::new(HashError {
                message: "Hash calculation failed".to_string(),
            }));
        };

        self.do_detect(hash)
    }

    fn check_reader(&mut self, input: &mut dyn Read) -> Result<DetectionResult, Box<dyn Error>> {
        let mut buffer = [0; READ_BUFFER_SIZE];

        let mut tlsh = A::new();
        while input.read(&mut buffer)? > 0 {
            tlsh.update(&buffer);
        }
        tlsh.finalize();
        let Some(hash) = tlsh.get_hash() else {
            return Err(Box::new(HashError {
                message: "Hash calculation failed".to_string(),
            }));
        };
        self.do_detect(hash)
    }
}

/// Implement the SIMBIoTA detection algorithm for [`DetectorImpl`].
///
/// The calculated hash is compared against all hashes in the database, a match is found when the diff
/// is less than a provided threshold.
pub struct CompareAgainstAllDetector<H>
where
    H: ComparableHash,
{
    compare_fn: Box<dyn Fn(H::ResultType) -> bool>,
    database: Box<dyn HashDatabase<H>>,
}
impl<'a, H> HashBasedDetector<'a, H> for CompareAgainstAllDetector<H>
where
    H: ComparableHash,
{
    fn do_detect(&mut self, hash: &H) -> Result<DetectionResult, Box<dyn Error>> {
        let mut compare_counter = 0;
        let start = Instant::now();
        let mut result = DetectionResult::NoMatch;
        for stored_hash in self.database.get_hashes() {
            let diff = hash.diff(stored_hash);
            compare_counter += 1;
            if (self.compare_fn)(diff) {
                result = DetectionResult::Match;
                break;
            }
        }
        let end = start.elapsed();
        let msc = end.as_micros() as f64 / compare_counter as f64;
        debug!(
            "compared against {} hashes in {:?} ({} us/comparision)",
            compare_counter, end, msc
        );
        Ok(result)
    }
}
impl<H> CompareAgainstAllDetector<H>
where
    H: ComparableHash,
{
    pub fn new(
        database: Box<dyn HashDatabase<H>>,
        comparator: Box<dyn Fn(H::ResultType) -> bool>,
    ) -> Self {
        Self {
            database,
            compare_fn: comparator,
        }
    }
}
