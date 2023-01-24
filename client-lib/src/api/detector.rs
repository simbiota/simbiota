use std::error::Error;
use std::io::Read;

/// The result of a detector check.
///
/// `Match` means the `Detector`'s comparator function returned true
/// for a comparison result. If no result was flagged by the comparator, `NoMatch` is returned.
/// The `Match` varian contains the value of the comparison diff as `value`.
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum DetectionResult {
    Match,
    NoMatch,
}

/// Generic detector interface
///
/// All SIMBIoTA code uses this when a detector is needed
pub trait Detector {
    /// Check whether the provided byte sequence results in a match using the
    /// detector.
    fn check_bytes(&mut self, bytes: &[u8]) -> Result<DetectionResult, Box<dyn Error>>;

    /// Checks whether the provided reader's content results in a match using the
    /// detector.
    fn check_reader(&mut self, reader: &mut dyn Read) -> Result<DetectionResult, Box<dyn Error>>;
}

/// A [`Detector`] implementation that uses a list of other detectors and weights
/// to calculate a result
///
/// The result is calculated using the following algorithm:
/// For each detector:
/// 1. Pass the input to the detector and collect the result
/// 2. Add the weight of the detector to the `matched` sum if the
/// detector matched, otherwise to the `not_matched sum`
/// Finally, return a match if the `matched` sum is greater than or equals
/// to the `not_matched` sum, else return no match.
#[derive(Default)]
pub struct WeightedDetector<'a> {
    detectors: Vec<(&'a mut dyn Detector, i32)>,
    max_weight: i32,
}

impl<'a> WeightedDetector<'a> {
    /// Add a detector to the list with the given weight.
    ///
    /// The weight is used for calculating the detection result using
    /// the formula documented above.
    pub fn add_detector(&mut self, detector: &'a mut dyn Detector, weight: i32) {
        self.detectors.push((detector, weight));
        self.max_weight += weight
    }
}

impl<'a> Detector for WeightedDetector<'a> {
    fn check_bytes(&mut self, bytes: &[u8]) -> Result<DetectionResult, Box<dyn Error>> {
        let mut match_sum = 0;
        let mut nomatch_sum = 0;
        for (detector, weight) in self.detectors.iter_mut() {
            let result = detector.check_bytes(bytes)?;
            if matches!(result, DetectionResult::Match) {
                match_sum += *weight;
            } else {
                nomatch_sum += *weight;
            }
        }

        Ok(if match_sum >= nomatch_sum {
            DetectionResult::Match
        } else {
            DetectionResult::NoMatch
        })
    }

    fn check_reader(&mut self, reader: &mut dyn Read) -> Result<DetectionResult, Box<dyn Error>> {
        let mut match_sum = 0;
        let mut nomatch_sum = 0;
        for (detector, weight) in self.detectors.iter_mut() {
            let result = detector.check_reader(reader)?;
            if matches!(result, DetectionResult::Match) {
                match_sum += *weight;
            } else {
                nomatch_sum += *weight;
            }
        }

        // Use >= here as a safety measure
        Ok(if match_sum >= nomatch_sum {
            DetectionResult::Match
        } else {
            DetectionResult::NoMatch
        })
    }
}
