#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use defmt::info;
use embassy_executor::Spawner;
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use embassy_time::Duration;
use esp_hal::clock::CpuClock;
use esp_hal::rng::Rng;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::uart::Uart;
use esp_hal::Blocking;
use picoserve::{AppRouter, AppWithStateBuilder as _};
use webserver_html::alloc::format;
use webserver_html::alloc::string::String;
use webserver_html::net::mqtt::mqtt_runner;
use webserver_html::net::web::{web_task_runner, AppState, MessageData};
use webserver_html::printer::ThermalPrinter;
use webserver_html::{
    net::{web::Application, wifi},
    printer,
};

use {esp_backtrace as _, esp_println as _};

// TODO move to separate file
type PrinterChannel = Channel<CriticalSectionRawMutex, MessageData, 8>;
type PrinterSender = Sender<'static, CriticalSectionRawMutex, MessageData, 8>;
type PrinterReceiver = Receiver<'static, CriticalSectionRawMutex, MessageData, 8>;

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

    static CHANNEL: PrinterChannel = Channel::new();

    // start_web_server(stack, &spawner, CHANNEL.sender()).await;

    let uart = Uart::new(peripherals.UART1, esp_hal::uart::Config::default()).unwrap();
    start_printer_service(uart, &spawner, CHANNEL.receiver()).await;

    let client_id = format!(
        "{:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
        mac_address[0],
        mac_address[1],
        mac_address[2],
        mac_address[3],
        mac_address[4],
        mac_address[5]
    );
    start_mqtt_service(&spawner, stack, rng, client_id).await;
}

/* -------------- WEB SERVER TASK -------------- */
const WEB_TASK_POOL_SIZE: usize = 2;

async fn start_web_server(stack: Stack<'static>, spawner: &Spawner, sender: PrinterSender) {
    let router = picoserve::make_static!(
        AppRouter<Application<PrinterSender>>,
        Application::new().build_app()
    );
    let config = picoserve::make_static!(
        picoserve::Config<Duration>,
        picoserve::Config::new(picoserve::Timeouts {
            start_read_request: Some(Duration::from_secs(5)),
            read_request: Some(Duration::from_secs(1)),
            write: Some(Duration::from_secs(1)),
            persistent_start_read_request: Some(Duration::from_secs(5))
        })
        .keep_connection_alive()
    );

    for id in 0..WEB_TASK_POOL_SIZE {
        spawner.must_spawn(web_task(id, stack, router, config, AppState { sender }));
    }
    info!("Web server initialized...");
}

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
async fn web_task(
    id: usize,
    stack: Stack<'static>,
    router: &'static AppRouter<Application<PrinterSender>>,
    config: &'static picoserve::Config<Duration>,
    state: AppState<PrinterSender>,
) {
    web_task_runner(id, stack, router, config, state).await;
}

/* -------------- THERMAL PRINTER TASK -------------- */
async fn start_printer_service(
    uart: Uart<'static, Blocking>,
    spawner: &Spawner,
    rx: PrinterReceiver,
) {
    let mut printer = printer::ThermalPrinter::new(uart).await;
    printer.print("Test Print, extra lines 12345678901234567890");

    spawner.must_spawn(print_task(printer, rx));
    info!("Printer initialized...")
}

#[embassy_executor::task]
async fn print_task(mut printer: ThermalPrinter<Uart<'static, Blocking>>, rx: PrinterReceiver) {
    loop {
        let data = rx.receive().await;
        printer.print(&data);
    }
}

/* -------------- MQTT SERVICE TASK -------------- */
async fn start_mqtt_service(spawner: &Spawner, stack: Stack<'static>, rng: Rng, client_id: String) {
    spawner.must_spawn(mqtt_task(stack, rng, client_id));
    info!("MQTT initialized...")
}

#[embassy_executor::task]
async fn mqtt_task(stack: Stack<'static>, rng: Rng, client_id: String) {
    mqtt_runner(stack, rng, &client_id).await;
}
