use rand_core::RngCore;

#[derive(Clone, Copy)]
pub struct Rng(esp_hal::rng::Rng);

impl From<esp_hal::rng::Rng> for Rng {
    fn from(value: esp_hal::rng::Rng) -> Self {
        Self(value)
    }
}

impl RngCore for Rng {
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.0.try_fill_bytes(dest)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.fill_bytes(dest)
    }
}
