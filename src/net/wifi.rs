use defmt::info;
use embassy_executor::Spawner;
use embassy_net::{DhcpConfig, Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};

use crate::{
    glue::{Wifi, WifiController, WifiInterface},
    mk_static,
};

const SSID: &str = env!("WIFI_SSID");
const PASSWORD: &str = env!("WIFI_PASSWORD");

pub async fn start_wifi(wifi: Wifi, spawner: &Spawner) -> (Stack<'static>, [u8; 6]) {
    let dhcp_config = DhcpConfig::default();
    let net_config = embassy_net::Config::dhcpv4(dhcp_config);

    // Init network stack
    let mac_address = wifi.mac_adderss();
    let seed = wifi.net_seed();
    let (interface, controller) = wifi.interface();
    let (stack, runner) = embassy_net::new(
        interface,
        net_config,
        mk_static!(StackResources<5>, StackResources::<5>::new()),
        seed,
    );

    spawner.spawn(connection(controller)).ok();
    spawner.spawn(net_task(runner)).ok();

    wait_for_connection(stack).await;

    (stack, mac_address)
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController) {
    info!("start connection task");
    info!("Device capabilities: {:?}", controller.capabilities());

    controller.connection_loop(SSID, PASSWORD).await;
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiInterface>) {
    runner.run().await
}

async fn wait_for_connection(stack: Stack<'_>) {
    info!("Waiting for link to be up");
    stack.wait_link_up().await;
    info!("Waiting to get IP address...");
    stack.wait_config_up().await;
    loop {
        if let Some(config) = stack.config_v4() {
            info!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
}
