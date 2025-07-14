#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
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

extern crate alloc;

use esp_wifi::EspWifiController;
use webserver_html as lib;
use webserver_html::web::{AppState, MAX_BODY_LEN};

esp_bootloader_esp_idf::esp_app_desc!();

pub static PRINT_CHANNEL: Channel<CriticalSectionRawMutex, heapless::String<MAX_BODY_LEN>, 8> =
    Channel::new();

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
    let esp_wifi_ctrl = &*lib::mk_static!(
        EspWifiController<'static>,
        esp_wifi::init(timer1.timer0, rng.clone(), peripherals.RADIO_CLK,).unwrap()
    );

    let stack = lib::wifi::start_wifi(esp_wifi_ctrl, peripherals.WIFI, rng, &spawner).await;

    let web_app = lib::web::WebApp::default();
    for id in 0..lib::web::WEB_TASK_POOL_SIZE {
        spawner.must_spawn(lib::web::web_task(
            id,
            stack,
            web_app.router,
            web_app.config,
            AppState {
                sender: PRINT_CHANNEL.sender(),
            },
        ));
    }
    info!("Web server started...");
    let mut uart = Uart::new(peripherals.UART1, esp_hal::uart::Config::default()).unwrap();

    lib::printer::initialize_printer(&mut uart).await;
    lib::printer::print_wrapped_upside_down(
        &mut uart,
        "Test Print, extra lines 12345678901234567890",
        5,
    );

    spawner.must_spawn(print_task(uart));
    info!("Printer initialized...")
}

#[embassy_executor::task]
pub async fn print_task(mut uart: Uart<'static, Blocking>) {
    let rx = PRINT_CHANNEL.receiver();
    loop {
        let data = rx.receive().await;
        lib::printer::print_wrapped_upside_down(&mut uart, &data, 10);
    }
}
