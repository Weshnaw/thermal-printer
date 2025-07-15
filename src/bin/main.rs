#![no_std]
#![no_main]

use core::marker::PhantomData;

use defmt::info;
use embassy_executor::Spawner;
use embassy_net::Stack;
use embassy_sync::channel::Channel;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Sender};
use embassy_time::Duration;
use esp_hal::clock::CpuClock;
use esp_hal::rng::Rng;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::uart::Uart;
use esp_hal::Blocking;
use esp_println as _;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

use picoserve::{AppRouter, AppWithStateBuilder as _};
use webserver_html::net::web::{web_task_runner, AppState, MessageData};
use webserver_html::{
    net::{web::Application, wifi},
    printer,
};

esp_bootloader_esp_idf::esp_app_desc!();

// TODO reconsider static channels
type PrinterSender = Sender<'static, CriticalSectionRawMutex, MessageData, 8>;
static PRINT_CHANNEL: Channel<CriticalSectionRawMutex, MessageData, 8> = Channel::new();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    // generator version: 0.3.1

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timer0 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    info!("Embassy initialized!");

    let timer1 = TimerGroup::new(peripherals.TIMG0);
    let rng = Rng::new(peripherals.RNG);

    let stack = wifi::start_wifi(
        timer1.timer0,
        peripherals.RADIO_CLK,
        peripherals.WIFI,
        rng,
        &spawner,
    )
    .await;

    start_web_server(stack, &spawner, PRINT_CHANNEL.sender()).await;

    let uart = Uart::new(peripherals.UART1, esp_hal::uart::Config::default()).unwrap();
    start_printer_service(uart, &spawner).await;
}

const WEB_TASK_POOL_SIZE: usize = 2;

async fn start_web_server(stack: Stack<'static>, spawner: &Spawner, sender: PrinterSender) {
    let router = picoserve::make_static!(
        AppRouter<Application<PrinterSender>>,
        Application(PhantomData).build_app()
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
    info!("Web server started...");
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

async fn start_printer_service(mut uart: Uart<'static, Blocking>, spawner: &Spawner) {
    printer::initialize_printer(&mut uart).await;
    printer::print_wrapped_upside_down(
        &mut uart,
        "Test Print, extra lines 12345678901234567890",
        5,
    );

    spawner.must_spawn(print_task(uart));
    info!("Printer initialized...")
}

#[embassy_executor::task]
async fn print_task(mut uart: Uart<'static, Blocking>) {
    let rx = PRINT_CHANNEL.receiver();
    loop {
        let data = rx.receive().await;
        printer::print_wrapped_upside_down(&mut uart, &data, 10);
    }
}
