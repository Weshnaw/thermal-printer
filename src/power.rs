use defmt::{debug, info, warn};
use embassy_executor::Spawner;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    watch::{Receiver, Sender, Watch},
};
use embassy_time::{Duration, Ticker};

use crate::glue::PowerMonitorADC;

const WATCHER_SIZE: usize = 2;
pub type PowerMonitorData = u16;
type PowerMonitorWatcher = Watch<CriticalSectionRawMutex, PowerMonitorData, WATCHER_SIZE>;
type PowerMonitorSender = Sender<'static, CriticalSectionRawMutex, PowerMonitorData, WATCHER_SIZE>;
pub type PowerMonitorReceiver =
    Receiver<'static, CriticalSectionRawMutex, PowerMonitorData, WATCHER_SIZE>;

type ShutdownWatcher = Watch<CriticalSectionRawMutex, ShutdownStatus, WATCHER_SIZE>;
type ShutdownSender = Sender<'static, CriticalSectionRawMutex, ShutdownStatus, WATCHER_SIZE>;
pub type ShutdownReceiver =
    Receiver<'static, CriticalSectionRawMutex, ShutdownStatus, WATCHER_SIZE>;

pub static SHUTDOWN_WATCHER: ShutdownWatcher = Watch::new();
pub static POWER_MONITOR_WATCHER: PowerMonitorWatcher = Watch::new();

#[embassy_executor::task]
async fn shutdown_task(service: ShutdownService) {
    service.run().await
}

pub fn start_power_monitor(monitor: PowerMonitorADC, spawner: &Spawner) {
    let shutdown = ShutdownService::new(monitor);
    spawner.must_spawn(shutdown_task(shutdown));
}

pub struct ShutdownService {
    monitor: PowerMonitorADC,
    monitor_sender: PowerMonitorSender,
    shutdown_sender: ShutdownSender,
}

// magic numbers based on quick manual calibration of adc
const NORMAL_POWER: u16 = 700;
const POWER_LOSS: u16 = 1_000;
const USB_POWER: u16 = 2_200;

impl ShutdownService {
    pub fn new(monitor: PowerMonitorADC) -> Self {
        Self {
            monitor,
            shutdown_sender: SHUTDOWN_WATCHER.sender(),
            monitor_sender: POWER_MONITOR_WATCHER.sender(),
        }
    }

    pub async fn run(mut self) {
        let mut status = ShutdownStatus::NormalPower;

        let mut ticker = Ticker::every(Duration::from_millis(50));
        loop {
            ticker.next().await;
            let adc_value = self.monitor.read_oneshot();

            debug!("Battery ADC: {}", adc_value);
            self.monitor_sender.send(adc_value);

            match (status, adc_value) {
                (ShutdownStatus::LowPower, 0..=NORMAL_POWER) => { 
                    info!("Power regained, returning to normal power state");
                    status = ShutdownStatus::NormalPower;
                    self.shutdown_sender.send(status);
                }
                (ShutdownStatus::NormalPower, POWER_LOSS..=USB_POWER) => {
                    warn!("Losing power, sending shutdown signal");
                    status = ShutdownStatus::LowPower;
                    self.shutdown_sender.send(status);
                }
                (_, _) => {
                    continue;
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum ShutdownStatus {
    LowPower,
    NormalPower,
}
