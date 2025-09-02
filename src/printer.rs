use alloc::{sync::Arc, vec::Vec};
use defmt::{debug, info};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};
use esp_hal::gpio::Input;

type MessageData = Arc<str>;
type PrinterChannel = Channel<CriticalSectionRawMutex, MessageData, 8>;
type PrinterSender = Sender<'static, CriticalSectionRawMutex, MessageData, 8>;
type PrinterReceiver = Receiver<'static, CriticalSectionRawMutex, MessageData, 8>;

static PRINTER_CHANNEL: PrinterChannel = Channel::new();
static MAX_CHARACTERS_PER_LINE: usize = 30;

pub async fn new<T: embedded_io_async::Write>(
    writer: T,
    dtr_pin: Input<'static>,
) -> (ThermalPrinterService<T>, ThermalPrinter) {
    let mut printer = ThermalPrinterService::new(writer, dtr_pin).await;
    printer
        .print("Initialized")
        .await;

    (printer, ThermalPrinter::new())
}

#[derive(Clone)]
pub struct ThermalPrinter {
    printer_tx: PrinterSender,
}

impl ThermalPrinter {
    pub fn new() -> Self {
        let printer_tx = PRINTER_CHANNEL.sender();

        ThermalPrinter { printer_tx }
    }

    pub async fn print(&self, buf: MessageData) {
        info!("Sending data: {}", buf);
        self.printer_tx.send(buf).await;
        info!("Data sent");
    }
}

impl Default for ThermalPrinter {
    fn default() -> Self {
        Self::new()
    }
}

// TODO; setup a configuration that lets this be changed dynamically via mqtt
pub struct ThermalPrinterService<T: embedded_io_async::Write> {
    serial_writer: T,
    printer_rx: PrinterReceiver,
    dtr_pin: Input<'static>,
}

impl<T: embedded_io_async::Write> ThermalPrinterService<T> {
    async fn new(serial_writer: T, dtr_pin: Input<'static>) -> Self {
        let printer_rx = PRINTER_CHANNEL.receiver();
        let mut device = Self {
            serial_writer,
            printer_rx,
            dtr_pin,
        };

        device.send_data(&[0x1B, b'@']).await; // ESC @
        device.send_data(&[0x1B, b'7', 15, 150, 250]).await; // print density
        device.send_data(&[0x1B, b'{', 0x01]).await; // 180° rotation

        device
    }

    async fn print(&mut self, text: &str) {
        info!("creating lines: {}", text);

        let mut lines = Vec::new();
        let mut remaining = text.trim();

        while !remaining.is_empty() {
            let take_len = core::cmp::min(MAX_CHARACTERS_PER_LINE, remaining.len());
            let slice = &remaining[..take_len];

            // Try to break at the last space within the slice
            let break_point = slice.rfind(' ').unwrap_or(take_len);
            let split_idx = if break_point == 0 {
                take_len
            } else {
                break_point
            };

            let (line, rest) = remaining.split_at(split_idx);
            lines.push(line.trim());

            remaining = rest.trim_start();
        }

        info!("Printing");
        for line in lines.into_iter().rev() {
            self.print_line(line).await;
        }

        info!("Print complete");
        self.advance_paper(2).await;
    }

    async fn advance_paper(&mut self, lines: usize) {
        debug!("Advancing: {} lines", lines);
        for _ in 0..lines {
            self.send_data(&[0x0A]).await; // LF
        }
    }

    async fn print_line(&mut self, line: &str) {
        debug!("Printing line: {}", line);

        self.send_data(line.as_bytes()).await;
        self.send_data(&[0x0A]).await; // LF
    }

    async fn send_data(&mut self, data: &[u8]) {
        self.dtr_pin.wait_for_high().await;
        self.serial_writer.write(data).await.unwrap();
    }

    // fn set_inverse(&mut self, enable: bool) {
    //     uart.write_all(&[0x1D, b'B', if enable { 1 } else { 0 }])
    //         .unwrap();
    // }
    //
    pub async fn run(mut self) {
        loop {
            let data = self.printer_rx.receive().await;
            info!("Received data: {}", data);
            self.print(&data).await;
        }
    }
}
