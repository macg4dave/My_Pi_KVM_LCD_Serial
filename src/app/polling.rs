use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{
    mpsc::{self, Receiver},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

use sysinfo::{DiskExt, System as InfoSystem, SystemExt};
use systemstat::{data::CpuLoad, data::Temperature, Platform, System as StatSystem};

/// Snapshot of the most-recent metric poll (CPU, memory, disk, temperature).
#[derive(Debug, Clone, PartialEq)]
pub struct PollSnapshot {
    pub cpu_percent: f32,
    pub mem_used_kb: u64,
    pub mem_total_kb: u64,
    pub disk_used_pct: f32,
    pub disk_available_kb: Option<u64>,
    pub temperature_c: Option<f32>,
}

/// Reports sent over the polling channel.
#[derive(Debug)]
pub enum PollEvent {
    Snapshot(PollSnapshot),
    Error(String),
}

/// Guard that keeps the poller thread alive until the flag is toggled.
pub struct PollingHandle {
    receiver: Receiver<PollEvent>,
    running: Arc<AtomicBool>,
}

impl PollingHandle {
    pub fn receiver(&self) -> &Receiver<PollEvent> {
        &self.receiver
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Drop for PollingHandle {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

/// Spawn the background poller that pushes snapshots at roughly `interval_ms`.
pub fn start_polling(interval_ms: u64, app_running: Arc<AtomicBool>) -> PollingHandle {
    let interval = Duration::from_millis(interval_ms.max(1));
    let (tx, rx) = mpsc::channel();
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    thread::Builder::new()
        .name("lifelinetty-poller".into())
        .spawn(move || match Poller::new() {
            Ok(mut poller) => {
                while app_running.load(Ordering::SeqCst) && running_clone.load(Ordering::SeqCst) {
                    let start = Instant::now();
                    let event = match poller.poll_once() {
                        Ok(snapshot) => PollEvent::Snapshot(snapshot),
                        Err(err) => PollEvent::Error(err),
                    };
                    let _ = tx.send(event);
                    let elapsed = start.elapsed();
                    if elapsed < interval {
                        thread::sleep(interval - elapsed);
                    }
                }
            }
            Err(err) => {
                let _ = tx.send(PollEvent::Error(err));
            }
        })
        .expect("failed to spawn poller thread");
    PollingHandle {
        receiver: rx,
        running,
    }
}

struct Poller {
    stats: StatSystem,
    sysinfo: InfoSystem,
    cpu_load: CpuLoad,
}

impl Poller {
    fn new() -> Result<Self, String> {
        let stats = StatSystem::new();
        let cpu_load = stats.cpu_load_aggregate().map_err(|e| e.to_string())?;
        Ok(Self {
            stats,
            sysinfo: InfoSystem::new(),
            cpu_load,
        })
    }

    fn poll_once(&mut self) -> Result<PollSnapshot, String> {
        let load = self.cpu_load.done().map_err(|e| e.to_string())?;
        let cpu_percent = ((1.0 - load.idle) * 100.0).clamp(0.0, 100.0);
        self.cpu_load = self.stats.cpu_load_aggregate().map_err(|e| e.to_string())?;
        self.sysinfo.refresh_memory();
        self.sysinfo.refresh_disks();
        let mem_used = self.sysinfo.used_memory();
        let mem_total = self.sysinfo.total_memory();
        let disk = self
            .sysinfo
            .disks()
            .iter()
            .find(|disk| disk.mount_point() == Path::new("/"))
            .or_else(|| self.sysinfo.disks().first());
        let (disk_used_pct, disk_available_kb) = if let Some(disk) = disk {
            let total = disk.total_space();
            let available = disk.available_space();
            let used_pct = if total > 0 {
                ((total.saturating_sub(available)) as f32 / total as f32) * 100.0
            } else {
                0.0
            };
            (used_pct.min(100.0), Some(available / 1024))
        } else {
            (0.0, None)
        };
        let temperature_c = self
            .stats
            .temperature()
            .ok()
            .and_then(|temp| temp.as_celsius());
        Ok(PollSnapshot {
            cpu_percent,
            mem_used_kb: mem_used,
            mem_total_kb: mem_total,
            disk_used_pct,
            disk_available_kb,
            temperature_c,
        })
    }
}
