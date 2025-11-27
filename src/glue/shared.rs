#[derive(Debug, Default, defmt::Format)]
pub struct Capabilities {
    client: bool,
    access_point: bool,
    ap_sta: bool,
}

impl Capabilities {
    pub fn builder() -> CapabilitiesBuilder {
        CapabilitiesBuilder(Self::default())
    }
}

pub struct CapabilitiesBuilder(Capabilities);

impl CapabilitiesBuilder {
    pub fn client_capable(mut self) -> CapabilitiesBuilder {
        self.0.client = true;

        self
    }
    pub fn access_point_capable(mut self) -> CapabilitiesBuilder {
        self.0.access_point = true;

        self
    }
    pub fn ap_sta_capable(mut self) -> CapabilitiesBuilder {
        self.0.ap_sta = true;

        self
    }

    pub fn build(self) -> Capabilities {
        self.0
    }
}
