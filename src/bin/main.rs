#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use defmt::info;
use embassy_executor::Spawner;
use esp_hal::clock::CpuClock;
use esp_hal::rng::Rng;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::uart::Uart;
use esp_hal::Blocking;
use webserver_html::alloc::format;
use webserver_html::alloc::rc::Rc;
use webserver_html::net::mqtt::MQTTService;
use webserver_html::net::web::WebService;
use webserver_html::printer::ThermalPrinterService;
use webserver_html::{net::wifi, printer};

use {esp_backtrace as _, esp_println as _};

esp_bootloader_esp_idf::esp_app_desc!();
#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timer0 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    info!("Embassy initialized...");

    let timer1 = TimerGroup::new(peripherals.TIMG0);
    let rng = Rng::new(peripherals.RNG);

    let (stack, mac_address) = wifi::start_wifi(
        timer1.timer0,
        peripherals.RADIO_CLK,
        peripherals.WIFI,
        rng,
        &spawner,
    )
    .await;

    info!("MAC Address: {:#x}", mac_address);

    let uart = Uart::new(peripherals.UART1, esp_hal::uart::Config::default()).unwrap();

    let (printer, printer_writer) = printer::new(uart).await;
    spawner.must_spawn(printer_task(printer));
    info!("Printer initialized...");

    let client_id = format!(
        "{:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
        mac_address[0],
        mac_address[1],
        mac_address[2],
        mac_address[3],
        mac_address[4],
        mac_address[5]
    );
    let mqtt = MQTTService::new(stack, rng, client_id);
    spawner.must_spawn(mqtt_task(mqtt));
    info!("MQTT initialized...");

    let web = Rc::new(WebService::new(stack, printer_writer).await);
    for id in 0..WEB_TASK_POOL_SIZE {
        spawner.must_spawn(web_task(id, web.clone()));
    }
    info!("Web Server initialized...");
}

/* -------------- WEB SERVER TASK -------------- */
const WEB_TASK_POOL_SIZE: usize = 2;
#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
async fn web_task(id: usize, runner: Rc<WebService>) {
    runner.run(id).await;
}

#[embassy_executor::task]
async fn mqtt_task(runner: MQTTService) {
    runner.run().await;
}

#[embassy_executor::task]
async fn printer_task(runner: ThermalPrinterService<Uart<'static, Blocking>>) {
    runner.run().await
}
