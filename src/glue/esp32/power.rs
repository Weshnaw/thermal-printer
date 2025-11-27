use defmt::warn;
use esp_hal::{
    Blocking,
    analog::adc::{Adc, AdcPin},
    peripherals::{ADC1, GPIO32},
};

pub struct PowerMonitorADC {
    pin: AdcPin<GPIO32<'static>, ADC1<'static>>,
    adc: Adc<'static, ADC1<'static>, Blocking>,
}

impl PowerMonitorADC {
    pub fn new(
        pin: AdcPin<GPIO32<'static>, ADC1<'static>>,
        adc: Adc<'static, ADC1<'static>, Blocking>,
    ) -> Self {
        Self { pin, adc }
    }

    pub fn read_oneshot(&mut self) -> u16 {
        match nb::block!(self.adc.read_oneshot(&mut self.pin)) {
            Ok(v) => v,
            Err(_) => {
                warn!("Failed to read shutdown ADC");
                0
            }
        }
    }
}
