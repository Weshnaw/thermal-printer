#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use defmt::{error, info};
use embassy_executor::Spawner;
use esp_hal::{
    analog::adc::{Adc, AdcConfig, Attenuation},
    gpio::{Input, InputConfig},
    interrupt::software::SoftwareInterruptControl,
    rng::Rng,
    system::Stack,
    timer::timg::TimerGroup,
    uart::AtCmdConfig,
};
use esp_hal::{clock::CpuClock, uart::Uart};
use esp_rtos::embassy::Executor;
use static_cell::StaticCell;
use webserver_html::prelude::*;

use {esp_alloc as _, esp_backtrace as _, esp_println as _};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    // init esp32
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 98767);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    info!("Embassy initialized!");

    // init second core

    // init power monitor peripherials

    // ADC2 cannot be used with wifi, so I would have to refactor this to be use some external ADC
    let adc_pin = peripherals.GPIO32;
    let mut adc_config = AdcConfig::new();
    let pin = adc_config.enable_pin(adc_pin, Attenuation::_11dB);
    let adc = Adc::new(peripherals.ADC1, adc_config);

    let power_monitor = PowerMonitorADC::new(pin, adc);

    let software_interrupt = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    static APP_CORE_STACK: StaticCell<Stack<8192>> = StaticCell::new();
    let app_core_stack = APP_CORE_STACK.init(Stack::new());
    esp_rtos::start_second_core(
        peripherals.CPU_CTRL,
        software_interrupt.software_interrupt0,
        software_interrupt.software_interrupt1,
        app_core_stack,
        move || {
            static EXECUTOR: StaticCell<Executor> = StaticCell::new();
            let executor = EXECUTOR.init(Executor::new());
            executor.run(|spawner| {
                start_power_monitor(power_monitor, &spawner);
            });
        },
    );

    info!("Second core initialized");

    // init wifi peripherials
    let radio_init = &*webserver_html::mk_static!(
        esp_radio::Controller<'static>,
        esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller")
    );
    let rng = Rng::new();

    let wifi = Wifi::new(radio_init, peripherals.WIFI, rng);

    let (stack, mac_address) = start_wifi(wifi, &spawner).await;
    info!("MAC Address: {:#x}", mac_address);

    // init printer peripherials
    let config = esp_hal::uart::Config::default()
        .with_baudrate(9600)
        .with_parity(esp_hal::uart::Parity::None)
        .with_data_bits(esp_hal::uart::DataBits::_8)
        .with_stop_bits(esp_hal::uart::StopBits::_1);
    // .with_rx(RxConfig::default().with_fifo_full_threshold(1024));

    let mut uart = match Uart::new(peripherals.UART2, config) {
        Ok(uart) => uart
            .with_rx(peripherals.GPIO17)
            .with_tx(peripherals.GPIO16)
            .into_async(),
        Err(e) => {
            error!("Failed to initialize printer uart: {:?}", e);
            panic!("Failed to initialize printer uart: {:?}", e)
        }
    };

    uart.set_at_cmd(AtCmdConfig::default().with_cmd_char(0x04));

    let input = Input::new(
        peripherals.GPIO14,
        InputConfig::default().with_pull(esp_hal::gpio::Pull::Down),
    );
    let printer = ThermalPrinter::new(uart, input);

    start_printer(printer, &spawner).await;

    start_mqtt_client(mac_address, stack, rng.into(), &spawner);

    start_web_host(stack, &spawner);
}
