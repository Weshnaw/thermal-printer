use defmt::{info, warn};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    watch::{Receiver, Sender, Watch},
};
use embassy_time::{Duration, Timer};
use esp_hal::{
    Blocking,
    analog::adc::{Adc, AdcChannel, AdcPin, RegisterAccess},
    gpio::AnalogPin,
};

const WATCHER_SIZE: usize = 2;
type ShutdownWatcher = Watch<CriticalSectionRawMutex, u8, WATCHER_SIZE>;
type ShutdownSender = Sender<'static, CriticalSectionRawMutex, u8, WATCHER_SIZE>;
pub type ShutdownReceiver = Receiver<'static, CriticalSectionRawMutex, u8, WATCHER_SIZE>;

pub static SHUTDOWN_WATCHER: ShutdownWatcher = Watch::new();

pub struct ShutdownService<P: AnalogPin + AdcChannel, ADCI: RegisterAccess + 'static> {
    adc: Adc<'static, ADCI, Blocking>,
    adc_pin: AdcPin<P, ADCI>,
    shutdown_sender: ShutdownSender,
}

impl<P: AnalogPin + AdcChannel, ADCI: RegisterAccess> ShutdownService<P, ADCI> {
    pub fn new(adc_pin: AdcPin<P, ADCI>, adc: Adc<'static, ADCI, Blocking>) -> Self {
        Self {
            adc,
            adc_pin,
            shutdown_sender: SHUTDOWN_WATCHER.sender(),
        }
    }

    pub async fn run(mut self) {
        loop {
            let adc_value: u16 = match nb::block!(self.adc.read_oneshot(&mut self.adc_pin)) {
                Ok(v) => v,
                Err(_) => {
                    warn!("Failed to read shutdown ADC");
                    continue;
                }
            };

            info!("Shutdown ADC: {}", adc_value);

            if adc_value < 10 {
                self.shutdown_sender.send(1);
            }

            Timer::after(Duration::from_secs(5)).await;
        }
    }
}
