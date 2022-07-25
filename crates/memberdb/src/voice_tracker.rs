use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serenity::prelude::TypeMapKey;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct VoiceTracker(HashMap<u64, Instant>);

impl VoiceTracker {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn track_all_voice(&mut self) -> impl Iterator<Item = (&u64, Duration)> {
        self.0.iter_mut().map(|(k, v)| {
            let new_instant = Instant::now();
            let dur = new_instant.saturating_duration_since(*v);
            *v = new_instant;
            (k, dur)
        })
    }

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

pub struct VoiceTrackerContainer;

impl TypeMapKey for VoiceTrackerContainer {
    type Value = Arc<Mutex<VoiceTracker>>;
}

impl VoiceTrackerContainer {
    pub fn new() -> Arc<Mutex<VoiceTracker>> {
        Arc::new(Mutex::new(VoiceTracker::new()))
    }
}
