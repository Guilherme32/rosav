use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

use super::TimeSeriesConfig;
use crate::spectrum::Limits;
use crate::svg_utils::*;

const MAX_TIME: i64 = 10 * 60 * 1000;
const CLEAN_THRESH: u64 = 1000;
const BAD_SCORE_THRESH: f64 = 9.0;

#[derive(Debug)]
pub struct Sequence<T> {
    pub alive: bool,
    pub values: Vec<T>,
}

impl<T> Sequence<T> {
    pub fn new(first_value: T) -> Sequence<T> {
        Sequence {
            alive: true,
            values: vec![first_value],
        }
    }

    pub fn push(&mut self, new_value: T) {
        self.values.push(new_value);
    }

    pub fn kill(&mut self) {
        self.alive = false;
    }
}

#[derive(Debug, Clone)]
pub struct TimedEntry {
    pub value: f64,
    pub timestamp: i64,
}

impl TimedEntry {
    pub fn new_now(value: f64) -> TimedEntry {
        TimedEntry {
            value,
            timestamp: Local::now().timestamp_millis(),
        }
    }
}

#[derive(Debug)]
pub struct TimeSeries {
    pub sequences: Vec<Sequence<TimedEntry>>,
    total_entries: u64,
    cleanup_counter: u64,
    newest_time: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchMatrix(Vec<Option<usize>>);

pub struct Duplicate {
    matrix_index: usize,
    entry_index: usize,
}

impl MatchMatrix {
    pub fn find_duplicates(&self) -> Vec<Duplicate> {
        let mut used_entries = HashSet::new();
        let mut duplicate_entries = HashSet::new();
        let mut duplicates = Vec::new();

        // Find duplicate entry indexes
        let _ = self
            .0
            .iter()
            .map(|entry_index| {
                if let Some(entry_index) = entry_index {
                    let entry_index = *entry_index;

                    if used_entries.contains(&entry_index) {
                        duplicate_entries.insert(entry_index);
                    } else {
                        used_entries.insert(entry_index);
                    }
                }
            })
            .collect::<Vec<_>>();

        // Find every matrix that points to those duplicate entry entries
        let _ = self
            .0
            .iter()
            .enumerate()
            .map(|(matrix_index, entry_index)| {
                if let Some(entry_index) = entry_index {
                    let entry_index = *entry_index;

                    if duplicate_entries.contains(&entry_index) {
                        duplicates.push(Duplicate {
                            matrix_index,
                            entry_index,
                        });
                    }
                }
            })
            .collect::<Vec<_>>();

        duplicates
    }

    pub fn recalculate(&self, last_entries: &[f64], new_entries: &[f64]) -> MatchMatrix {
        if last_entries.len() != self.0.len() {
            return MatchMatrix(vec![]);
        }

        let locked_indexes: HashSet<usize> = self.0.iter().flatten().copied().collect();

        let new_matrix = last_entries
            .iter()
            .zip(&self.0)
            .map(|(old_entry, old_index)| {
                old_index.map_or_else(
                    || get_closest_entry(*old_entry, new_entries, &locked_indexes),
                    Some,
                )
            })
            .collect();

        Self(new_matrix)
    }

    pub fn lock(&self, duplicate: Duplicate) -> MatchMatrix {
        let mut new_matrix = self.clone();
        new_matrix.0[duplicate.matrix_index] = Some(duplicate.entry_index);

        new_matrix
    }

    pub fn calculate_score(&self, last_entries: &[f64], new_entries: &[f64]) -> f64 {
        if last_entries.len() != self.0.len() {
            println!("Shouldnt reach here\n\n");
            return -1.0;
        }

        (0..last_entries.len())
            .filter(|i| self.0[*i].is_some())
            .map(|i| (new_entries[self.0[i].unwrap()] - last_entries[i]).abs())
            .map(|dist| dist * 1e9)
            .map(|dist| dist.powi(2))
            .fold(0.0, |acc, x| acc + x)
    }

    pub fn remove_bad_matches(&self, last_entries: &[f64], new_entries: &[f64]) -> MatchMatrix {
        if last_entries.len() != self.0.len() {
            println!("Shouldnt reach here\n\n");
            return MatchMatrix(vec![]);
        }

        let new_matrix = last_entries
            .iter()
            .zip(&self.0)
            .map(|(last_entry, index)| {
                (*index).filter(|&index| (last_entry - new_entries[index]).abs() < BAD_SCORE_THRESH)
            })
            .collect();

        Self(new_matrix)
    }
}

fn get_closest_entry(
    old_entry: f64,
    new_entries: &[f64],
    locked_indexes: &HashSet<usize>,
) -> Option<usize> {
    let mut best_match = None;

    let _ = new_entries
        .iter()
        .enumerate()
        .filter(|(i, _new_entry)| !locked_indexes.contains(i))
        .map(|(i, new_entry)| {
            if best_match.is_none() {
                best_match = Some(i);
                return;
            }
            let last_best = best_match.unwrap();

            if (new_entry - old_entry).abs() < (new_entries[last_best] - old_entry).abs() {
                best_match = Some(i);
            }
        })
        .collect::<Vec<_>>();

    best_match
}

// Both real and heuristic
#[derive(Debug)]
struct Solution {
    score: f64,
    match_matrix: MatchMatrix,
}

#[derive(Debug, PartialEq)]
struct ScoredLock {
    heuristic_score: f64,
    lock_matrix: MatchMatrix,
}

const MAX_ATTEMPTS: usize = 100;
pub fn calculate_match_matrix(last_entries: &[f64], new_entries: &[f64]) -> MatchMatrix {
    // Startup
    let empty_lock = MatchMatrix(vec![None; last_entries.len()]);
    let mut possible_locks: VecDeque<ScoredLock> = VecDeque::new();
    possible_locks.push_back(ScoredLock {
        lock_matrix: empty_lock,
        heuristic_score: 0.0,
    });
    let mut attempted_locks: Vec<ScoredLock> = vec![];

    let mut best_solution = Solution {
        match_matrix: MatchMatrix(vec![None; last_entries.len()]),
        score: f64::INFINITY,
    };

    for _ in 0..MAX_ATTEMPTS {
        // Get the oldest possible lock
        let scored_lock = possible_locks.pop_front();
        let scored_lock = if let Some(scored_lock) = scored_lock {
            scored_lock
        } else {
            break;
        };

        // Check the heuristic score to see if it is worth analysing
        if scored_lock.heuristic_score > best_solution.score {
            continue;
        }

        // Try to solve with the chosen lock
        let lock_matrix = &scored_lock.lock_matrix;

        let partial_solve = lock_matrix.recalculate(last_entries, new_entries);
        let partial_score = partial_solve.calculate_score(last_entries, new_entries);
        let duplicates = partial_solve.find_duplicates();

        // If there are no duplicates, it counts as a possible real solution
        // Check it it should be the new best one
        if (duplicates.is_empty()) && (partial_score < best_solution.score) {
            best_solution = Solution {
                score: partial_score,
                match_matrix: partial_solve,
            };
        }

        // Add new possible locks fixating each possible duplicate point
        for duplicate in duplicates {
            let new_lock = lock_matrix.lock(duplicate);
            let new_lock = ScoredLock {
                heuristic_score: partial_score,
                lock_matrix: new_lock,
            };

            if !possible_locks.contains(&new_lock) && !attempted_locks.contains(&new_lock) {
                possible_locks.push_back(new_lock);
            }
        }

        // Add the used lock to the attempted pile
        attempted_locks.push(scored_lock);
    }

    if best_solution.score < (BAD_SCORE_THRESH / last_entries.len() as f64) {
        best_solution.match_matrix
    } else {
        // Break the cycle if it is dire (Result of noise)
        MatchMatrix(vec![None; last_entries.len()])
    }
}

impl TimeSeries {
    pub fn empty() -> TimeSeries {
        TimeSeries {
            sequences: vec![],
            total_entries: 0,
            cleanup_counter: 0,
            newest_time: Local::now().timestamp_millis(),
        }
    }

    pub fn push_batch(&mut self, batch: &[TimedEntry]) {
        // Prepare values for the matching algorithm
        let (index_map, last_entries): (Vec<usize>, Vec<f64>) = self
            .sequences
            .iter()
            .enumerate()
            .filter(|(_i, sequence)| sequence.alive)
            .map(|(i, sequence)| {
                (
                    i,
                    sequence
                        .values
                        .last()
                        .expect("Sequences should always be started with one element")
                        .value,
                )
            })
            .unzip();

        let new_entries: Vec<f64> = batch.iter().map(|entry| entry.value).collect();
        let match_matrix = calculate_match_matrix(&last_entries, &new_entries);

        // Update old sequences with matches
        for (i, matched_entry) in match_matrix.0.iter().enumerate() {
            let sequence_i = index_map[i];

            if let Some(matched_entry) = matched_entry {
                self.sequences[sequence_i].push(batch[*matched_entry].clone());
                self.total_entries += 1;
                self.cleanup_counter += 1;
            } else {
                self.sequences[sequence_i].alive = false;
            }
        }

        // Create new sequences for unmatched
        let _ = (0..batch.len())
            .filter(|i| !match_matrix.0.contains(&Some(*i)))
            .map(|i| {
                let new_sequence = Sequence::new(batch[i].clone());
                self.sequences.push(new_sequence);
                self.total_entries += 1;
                self.cleanup_counter += 1;
            })
            .collect::<Vec<_>>();

        // Update newest time
        self.newest_time = if let Some(new_entry) = batch.last() {
            new_entry.timestamp
        } else {
            Local::now().timestamp_millis()
        };

        self.clean_old();
    }

    fn clean_old(&mut self) {
        if self.cleanup_counter < CLEAN_THRESH {
            return;
        }
        self.cleanup_counter = 0;

        let current_time = Local::now().timestamp_millis();
        let mut removed = 0;

        for sequence in self.sequences.iter_mut() {
            sequence.values.retain(|entry| {
                let retain = current_time - entry.timestamp < MAX_TIME;
                if !retain {
                    removed += 1;
                }
                retain
            });
        }

        self.sequences
            .retain(|sequence| !sequence.values.is_empty());
    }

    pub fn to_path(&self, svg_limits: (u32, u32), graph_limits: &Limits) -> Vec<String> {
        let graph_limits = GraphLimits {
            x: graph_limits.wavelength,
            y: (-1000.0 * 60.0 * 5.0, 0.0), // 5 mins in ms
        };
        let now = self.newest_time;

        self.sequences
            .iter()
            .map(|sequence| {
                let points: Vec<(f64, f64)> = sequence
                    .values
                    .iter()
                    .map(|value| (value.value, (value.timestamp - now) as f64))
                    .collect();

                bezier_path(&points, svg_limits, &graph_limits)
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct TimeSeriesGroup {
    pub valleys: TimeSeries,
    pub valley_means: TimeSeries,
    pub peaks: TimeSeries,
    pub peak_means: TimeSeries,
}

impl TimeSeriesGroup {
    pub fn empty() -> TimeSeriesGroup {
        TimeSeriesGroup {
            valleys: TimeSeries::empty(),
            valley_means: TimeSeries::empty(),
            peaks: TimeSeries::empty(),
            peak_means: TimeSeries::empty(),
        }
    }

    pub fn to_path(
        &self,
        svg_limits: (u32, u32),
        graph_limits: &Limits,
        config: &TimeSeriesConfig,
    ) -> TimeSeriesGroupPaths {
        let valleys = if config.draw_valleys {
            self.valleys.to_path(svg_limits, graph_limits)
        } else {
            vec![]
        };

        let valley_means = if config.draw_valley_means {
            self.valley_means.to_path(svg_limits, graph_limits)
        } else {
            vec![]
        };

        let peaks = if config.draw_peaks {
            self.peaks.to_path(svg_limits, graph_limits)
        } else {
            vec![]
        };

        let peak_means = if config.draw_peak_means {
            self.peak_means.to_path(svg_limits, graph_limits)
        } else {
            vec![]
        };

        TimeSeriesGroupPaths {
            valleys,
            valley_means,
            peaks,
            peak_means,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TimeSeriesGroupPaths {
    pub valleys: Vec<String>,
    pub valley_means: Vec<String>,
    pub peaks: Vec<String>,
    pub peak_means: Vec<String>,
}

impl TimeSeriesGroupPaths {
    pub fn empty() -> TimeSeriesGroupPaths {
        TimeSeriesGroupPaths {
            valleys: vec![],
            valley_means: vec![],
            peaks: vec![],
            peak_means: vec![],
        }
    }
}
