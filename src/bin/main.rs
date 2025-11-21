#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use defmt::info;
use embassy_executor::Spawner;
use esp_hal::Async;
use esp_hal::analog::adc::{Adc, AdcConfig, Attenuation};
use esp_hal::gpio::{Input, InputConfig};
use esp_hal::peripherals::{ADC1, GPIO32};
use esp_hal::rng::Rng;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::uart::AtCmdConfig;
use esp_hal::{clock::CpuClock, uart::Uart};
// use webserver_html::net::mqtt::MQTTService;
use webserver_html::net::web::WebService;
use webserver_html::printer::ThermalPrinterService;
use webserver_html::shutdown::ShutdownService;

use {esp_backtrace as _, esp_println as _};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 98767);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    info!("Embassy initialized!");

    let radio_init = &*webserver_html::mk_static!(
        esp_radio::Controller<'static>,
        esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller")
    );
    let rng = Rng::new();

    let (stack, mac_address) =
        webserver_html::net::wifi::start_wifi(radio_init, peripherals.WIFI, rng, &spawner).await;
    info!("MAC Address: {:#x}", mac_address);
    let config = esp_hal::uart::Config::default()
        .with_baudrate(9600)
        .with_parity(esp_hal::uart::Parity::None)
        .with_data_bits(esp_hal::uart::DataBits::_8)
        .with_stop_bits(esp_hal::uart::StopBits::_1);
    // .with_rx(RxConfig::default().with_fifo_full_threshold(1024));

    let mut uart = Uart::new(peripherals.UART2, config)
        .unwrap()
        .with_rx(peripherals.GPIO17)
        .with_tx(peripherals.GPIO16)
        .into_async();
    uart.set_at_cmd(AtCmdConfig::default().with_cmd_char(0x04));

    let input = Input::new(
        peripherals.GPIO14,
        InputConfig::default().with_pull(esp_hal::gpio::Pull::Down),
    );

    let (printer, printer_writer) = webserver_html::printer::new(uart, input).await;
    spawner.must_spawn(printer_task(printer));
    info!("Printer initialized...");

    let web =
        &*webserver_html::mk_static!(WebService, WebService::new(stack, printer_writer).await);
    for id in 0..WEB_TASK_POOL_SIZE {
        spawner.must_spawn(web_task(id, web));
    }
    info!("Web Server initialized...");
    // spawner.must_spawn(status_task());

    // ADC2 cannot be used with wifi, so I would have to refactor this to be use some external ADC
    let adc_pin = peripherals.GPIO32;
    let mut adc_config = AdcConfig::new();
    let pin = adc_config.enable_pin(adc_pin, Attenuation::_11dB);
    let adc = Adc::new(peripherals.ADC1, adc_config);
    let shutdown = ShutdownService::new(pin, adc);
    spawner.must_spawn(shutdown_task(shutdown));

    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(1)).await;
    }
}

const BUFFER_SIZE: usize = 1024;
const WEB_TASK_POOL_SIZE: usize = 2;
#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
pub async fn web_task(id: usize, service: &'static WebService) -> ! {
    let mut rx_buffer = [0u8; BUFFER_SIZE];
    let mut tx_buffer = [0u8; BUFFER_SIZE];
    let mut http_buffer = [0u8; BUFFER_SIZE * 2];

    service
        .run(id, &mut rx_buffer, &mut tx_buffer, &mut http_buffer)
        .await
}

// #[embassy_executor::task]
// async fn mqtt_task(service: MQTTService) {
//     service.run().await;
// }

#[embassy_executor::task]
async fn printer_task(service: ThermalPrinterService<Uart<'static, Async>>) {
    service.run().await
}

// #[embassy_executor::task]
// async fn status_task() -> ! {
//     status_runner().await
// }

#[embassy_executor::task]
async fn shutdown_task(service: ShutdownService<GPIO32<'static>, ADC1<'static>>) {
    service.run().await
}
