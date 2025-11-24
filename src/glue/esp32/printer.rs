use defmt::{debug, warn};
use esp_hal::{Async, gpio::Input, uart::Uart};

pub struct ThermalPrinter {
    uart: Uart<'static, Async>,
    dtr_pin: Input<'static>,
}

impl ThermalPrinter {
    pub fn new(uart: Uart<'static, Async>, dtr_pin: Input<'static>) -> Self {
        Self { uart, dtr_pin }
    }

    pub async fn send_data(&mut self, data: &[u8]) {
        self.dtr_pin.wait_for_high().await;
        match self.uart.write_async(data).await {
            Ok(written_bytes) => debug!(
                "{} bytes sent to thermal printer succesfully",
                written_bytes
            ),
            Err(e) => warn!("Thermal printer write failed with: {:?}", e),
        }
    }
}
