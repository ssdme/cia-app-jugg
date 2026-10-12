use serde::{Deserialize, Serialize};

/// Source IDs address the immutable path list submitted with this job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRange {
    pub source_id: usize,
    pub start: f64,
    pub duration: f64,
}

pub fn validate_ranges(ranges: &[SourceRange], durations: &[f64]) -> Result<(), String> {
    if ranges.is_empty() {
        return Err("No source ranges selected".into());
    }
    for range in ranges {
        let duration = durations
            .get(range.source_id)
            .ok_or("Source range references an unknown source")?;
        if !range.start.is_finite()
            || !range.duration.is_finite()
            || range.start < 0.0
            || range.duration <= 0.0
            || range.start + range.duration > duration + 0.051
        {
            return Err(format!(
                "Invalid interval for source {}",
                range.source_id + 1
            ));
        }
    }
    Ok(())
}

/// Older plans use a concatenated input timeline. Split each interval at file
/// boundaries rather than applying its global offset to an individual file.
pub fn migrate_intervals(
    intervals: &[(f64, f64)],
    durations: &[f64],
) -> Result<Vec<SourceRange>, String> {
    let total: f64 = durations.iter().sum();
    let mut result = Vec::new();
    for &(start, duration) in intervals {
        if !start.is_finite()
            || !duration.is_finite()
            || start < 0.0
            || duration <= 0.0
            || start + duration > total + 0.051
        {
            return Err("Legacy source interval does not match the submitted sources".into());
        }
        let mut offset = 0.0;
        for (source_id, &source_duration) in durations.iter().enumerate() {
            let local_start = start.max(offset) - offset;
            let local_end = (start + duration).min(offset + source_duration) - offset;
            if local_end > local_start {
                result.push(SourceRange {
                    source_id,
                    start: local_start,
                    duration: local_end - local_start,
                });
            }
            offset += source_duration;
        }
    }
    validate_ranges(&result, durations)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn later_source_uses_local_time_and_crossing_is_split() {
        let ranges = migrate_intervals(&[(20.0, 5.0), (8.0, 5.0)], &[10.0, 10.0, 10.0]).unwrap();
        assert_eq!(
            ranges[0],
            SourceRange {
                source_id: 2,
                start: 0.0,
                duration: 5.0
            }
        );
        assert_eq!(
            ranges[1],
            SourceRange {
                source_id: 0,
                start: 8.0,
                duration: 2.0
            }
        );
        assert_eq!(
            ranges[2],
            SourceRange {
                source_id: 1,
                start: 0.0,
                duration: 3.0
            }
        );
        assert!(migrate_intervals(&[(20.0, 5.0)], &[10.0]).is_err());
    }
}
