use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serenity::prelude::TypeMapKey;
use tokio::sync::Mutex;

#[derive(Debug)]
/// Tracks the voice chat duration of discord members.
pub struct VoiceTracker(HashMap<u64, Instant>);

impl VoiceTracker {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Get all tracked vc durations
    pub fn track_all_voice(&mut self) -> impl Iterator<Item = (&u64, Duration)> {
        self.0.iter_mut().map(|(k, v)| {
            let new_instant = Instant::now();
            let dur = new_instant.saturating_duration_since(*v);
            *v = new_instant;
            (k, dur)
        })
    }

    /// Get the vc duration of discord member, if there is none, begin duration tracking and return
    /// `None`
    pub fn track_voice(&mut self, id: &u64) -> Option<Duration> {
        match self.0.get_mut(id) {
            Some(instant) => {
                let new_instant = Instant::now();
                let dur = new_instant.saturating_duration_since(*instant);
                *instant = new_instant;
                Some(dur)
            }
            None => {
                self.0.insert(*id, Instant::now());
                None
            }
        }
    }

    /// Stop the duration tracking of discord member, and returns tracked duration if there is any
    pub fn untrack_voice(&mut self, id: &u64) -> Option<Duration> {
        if let Some(instant) = self.0.get(id) {
            let dur = Instant::now().saturating_duration_since(*instant);
            self.0.remove(id);
            Some(dur)
        } else {
            None
        }
    }
}

/// Bot data key for `VoiceTracker`
pub struct VoiceTrackerContainer;

impl TypeMapKey for VoiceTrackerContainer {
    type Value = Arc<Mutex<VoiceTracker>>;
}

impl VoiceTrackerContainer {
    /// Create a new `VoiceTracker` container
    pub fn new() -> Arc<Mutex<VoiceTracker>> {
        Arc::new(Mutex::new(VoiceTracker::new()))
    }
}
