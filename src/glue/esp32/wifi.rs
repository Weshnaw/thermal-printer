use defmt::info;
use embassy_net::driver::Driver;
use embassy_time::{Duration, Timer};
use esp_hal::rng::Rng;
use esp_radio::wifi::{
    ClientConfig, ModeConfig, ScanConfig, WifiController as EspWifiController, WifiDevice,
    WifiEvent, WifiStaState,
};

use crate::glue::shared::Capabilities;

pub struct Wifi {
    wifi_controller: EspWifiController<'static>,
    wifi_device: WifiDevice<'static>,
    net_seed: u64,
}

impl Wifi {
    pub fn new(
        radio_init: &'static esp_radio::Controller<'static>,
        wifi: esp_hal::peripherals::WIFI<'static>,
        rng: Rng,
    ) -> Self {
        let (wifi_controller, interfaces) =
            esp_radio::wifi::new(radio_init, wifi, Default::default())
                .expect("Failed to initialize Wi-Fi controller");

        let wifi_device = interfaces.sta;
        let net_seed = rng.random() as u64 | ((rng.random() as u64) << 32);

        Self {
            wifi_controller,
            wifi_device,
            net_seed,
        }
    }

    pub fn mac_adderss(&self) -> [u8; 6] {
        self.wifi_device.mac_address()
    }

    pub fn net_seed(&self) -> u64 {
        self.net_seed
    }

    pub fn interface(self) -> (WifiInterface, WifiController) {
        (
            WifiInterface(self.wifi_device),
            WifiController(self.wifi_controller),
        )
    }
}

pub struct WifiInterface(WifiDevice<'static>);

impl Driver for WifiInterface {
    type RxToken<'a>
        = esp_radio::wifi::WifiRxToken
    where
        Self: 'a;
    type TxToken<'a>
        = esp_radio::wifi::WifiTxToken
    where
        Self: 'a;

    fn receive(
        &mut self,
        cx: &mut core::task::Context,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        self.0.receive(cx)
    }

    fn transmit(&mut self, cx: &mut core::task::Context) -> Option<Self::TxToken<'_>> {
        self.0.transmit(cx)
    }

    fn link_state(&mut self, cx: &mut core::task::Context) -> embassy_net::driver::LinkState {
        self.0.link_state(cx)
    }

    fn capabilities(&self) -> embassy_net::driver::Capabilities {
        self.0.capabilities()
    }

    fn hardware_address(&self) -> embassy_net::driver::HardwareAddress {
        self.0.hardware_address()
    }
}

pub struct WifiController(EspWifiController<'static>);

impl WifiController {
    pub fn capabilities(&self) -> Capabilities {
        Capabilities::builder()
            .access_point_capable()
            .client_capable()
            .ap_sta_capable()
            .build()
    }

    pub async fn connection_loop(&mut self, ssid: &str, password: &str) {
        loop {
            if esp_radio::wifi::sta_state() == WifiStaState::Connected {
                // wait until we're no longer connected
                self.0.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }

            if !matches!(self.0.is_started(), Ok(true)) {
                let client_config = ModeConfig::Client(
                    ClientConfig::default()
                        .with_ssid(ssid.into())
                        .with_password(password.into()),
                );
                self.0.set_config(&client_config).unwrap();
                info!("Starting wifi");
                self.0.start_async().await.unwrap();
                info!("Wifi started!");

                info!("Scan");
                let scan_config = ScanConfig::default().with_max(10);
                let result = self.0.scan_with_config_async(scan_config).await.unwrap();
                for ap in result {
                    info!("{:?}", ap);
                }
            }
            info!("About to connect...");

            match self.0.connect_async().await {
                Ok(_) => info!("Wifi connected!"),
                Err(e) => {
                    info!("Failed to connect to wifi: {:?}", e);
                    Timer::after(Duration::from_millis(5000)).await
                }
            }
        }
    }
}
