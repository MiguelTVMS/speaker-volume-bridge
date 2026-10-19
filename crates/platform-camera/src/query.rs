//! Bounded native activity queries. Every query owns a fresh report inventory.
use super::{inventory::Inventory, worker_observation_unavailable};
use speaker_volume_bridge_domain::camera::CameraObservation;
use std::sync::{Arc, Mutex};

pub trait QuerySource {
    type Key: Ord + Clone;
    fn devices(&mut self) -> Option<Vec<Self::Key>>;
    fn start(&mut self, inventory: Arc<Mutex<Inventory<Self::Key>>>) -> bool;
    /// True only after the complete observation window; false on cancellation.
    fn wait(&mut self) -> bool;
    fn stop(&mut self) -> bool;
}

pub fn sample<S: QuerySource>(source: &mut S) -> CameraObservation {
    let inventory = Arc::new(Mutex::new(Inventory::default()));
    let keys = source.devices().map(|mut keys| {
        keys.sort_unstable();
        keys.dedup();
        keys
    });
    let started = keys.as_ref().is_some_and(|keys| {
        inventory.lock().unwrap().reconcile(keys.clone());
        source.start(Arc::clone(&inventory))
    });
    let completed = source.wait();
    let stopped = source.stop();
    let current = source.devices().map(|mut keys| {
        keys.sort_unstable();
        keys.dedup();
        keys
    });
    if started && completed && stopped && current == keys {
        inventory.lock().map_or_else(
            |_| worker_observation_unavailable(),
            |mut inventory| inventory.finish_window(),
        )
    } else {
        worker_observation_unavailable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use speaker_volume_bridge_domain::camera::CameraActivity;

    #[allow(clippy::struct_excessive_bools)] // Independent native failure injections in the test source.
    struct Fake {
        inventory: Option<Arc<Mutex<Inventory<u32>>>>,
        active: bool,
        completed: bool,
        healthy: bool,
        start_ok: bool,
        stop_ok: bool,
        hotplug: bool,
        enumeration_ok: bool,
        log: Vec<&'static str>,
    }
    impl QuerySource for Fake {
        type Key = u32;
        fn devices(&mut self) -> Option<Vec<u32>> {
            self.log.push("enumerate");
            self.enumeration_ok.then(|| {
                if self.hotplug && self.log.len() > 1 {
                    vec![1, 2, 3]
                } else {
                    vec![1, 2]
                }
            })
        }
        fn start(&mut self, inventory: Arc<Mutex<Inventory<u32>>>) -> bool {
            self.log.push("start");
            self.inventory = Some(inventory);
            self.start_ok
        }
        fn wait(&mut self) -> bool {
            self.log.push("wait");
            if let Some(inventory) = &self.inventory {
                let mut inventory = inventory.lock().unwrap();
                if self.active {
                    inventory.update(&1, true);
                }
                if !self.healthy {
                    inventory.fail();
                }
            }
            self.completed
        }
        fn stop(&mut self) -> bool {
            self.log.push("stop");
            self.stop_ok
        }
    }
    impl Default for Fake {
        fn default() -> Self {
            Self {
                inventory: None,
                active: false,
                completed: true,
                healthy: true,
                start_ok: true,
                stop_ok: true,
                hotplug: false,
                enumeration_ok: true,
                log: Vec::new(),
            }
        }
    }
    #[test]
    fn unsuccessful_or_hotplugged_queries_never_publish_inactivity() {
        for failure in 0..5 {
            let mut source = Fake::default();
            match failure {
                0 => source.start_ok = false,
                1 => source.stop_ok = false,
                2 => source.enumeration_ok = false,
                3 => source.hotplug = true,
                _ => source.completed = false,
            }
            assert_eq!(sample(&mut source).activity, CameraActivity::Unknown);
        }
    }
    #[test]
    fn bounded_queries_detect_idle_and_clear_previous_active_without_reports() {
        let mut source = Fake::default();
        assert_eq!(sample(&mut source).activity, CameraActivity::Inactive);
        assert_eq!(
            source.log,
            ["enumerate", "start", "wait", "stop", "enumerate"]
        );
        source.active = true;
        assert_eq!(sample(&mut source).activity, CameraActivity::Active);
        let old_callback = source.inventory.clone().unwrap();
        source.active = false;
        assert_eq!(sample(&mut source).activity, CameraActivity::Inactive);
        old_callback.lock().unwrap().update(&1, true);
        assert_eq!(
            source
                .inventory
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .observation()
                .activity,
            CameraActivity::Inactive
        );
        assert_eq!(sample(&mut source).activity, CameraActivity::Inactive);
        source.healthy = false;
        assert_eq!(sample(&mut source).activity, CameraActivity::Unknown);
        source.healthy = true;
        source.completed = false;
        assert_eq!(sample(&mut source).activity, CameraActivity::Unknown);
    }
}
