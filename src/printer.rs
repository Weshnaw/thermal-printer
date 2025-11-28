use core::str::FromStr as _;

use alloc::vec::Vec;
use defmt::{debug, info, warn};
use embassy_executor::Spawner;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};

use crate::glue::ThermalPrinter;

const CHANNEL_SIZE: usize = 8;
pub const DATA_SIZE: usize = 2048;
pub type MessageData = heapless::String<DATA_SIZE>;
type PrinterChannel = Channel<CriticalSectionRawMutex, MessageData, CHANNEL_SIZE>;
type PrinterSender = Sender<'static, CriticalSectionRawMutex, MessageData, CHANNEL_SIZE>;
type PrinterReceiver = Receiver<'static, CriticalSectionRawMutex, MessageData, CHANNEL_SIZE>;

static PRINTER_CHANNEL: PrinterChannel = Channel::new();
static MAX_CHARACTERS_PER_LINE: usize = 30;

pub async fn start_printer(printer: ThermalPrinter, spawner: &Spawner) {
    let printer = ThermalPrinterService::new(printer).await;

    spawner.must_spawn(printer_task(printer));
    info!("Printer initialized...");
}

#[embassy_executor::task]
async fn printer_task(service: ThermalPrinterService) {
    service.run().await
}

#[derive(Clone)]
pub struct PrinterWriter {
    printer_tx: PrinterSender,
}

impl PrinterWriter {
    pub fn new() -> Self {
        let printer_tx = PRINTER_CHANNEL.sender();

        PrinterWriter { printer_tx }
    }

    pub async fn chunk_print(&self, payload: &str) {
        let mut offset: usize = 0;
        while offset < payload.len() {
            let page = (DATA_SIZE + offset).min(payload.len());
            let slice = &payload[offset..page];
            let message: MessageData = heapless::String::from_str(slice).unwrap();
            self.print(message).await;
            offset = page;
        }
    }

    pub async fn print(&self, buf: MessageData) {
        info!("Sending data: {}", buf);
        self.printer_tx.send(buf).await;
        info!("Data sent");
    }
}

impl Default for PrinterWriter {
    fn default() -> Self {
        Self::new()
    }
}

struct ThermalPrinterService {
    printer: ThermalPrinter,
    printer_rx: PrinterReceiver,
}

impl ThermalPrinterService {
    async fn new(mut printer: ThermalPrinter) -> Self {
        printer.send_data(&[0x1B, b'@']).await; // ESC @
        printer.send_data(&[0x1B, b'7', 15, 150, 250]).await; // print density
        printer.send_data(&[0x1B, b'{', 0x01]).await; // 180° rotation

        let printer_rx = PRINTER_CHANNEL.receiver();

        Self {
            printer,
            printer_rx,
        }
    }
    async fn print(&mut self, text: &[u8]) {
        debug!("creating lines: {}", text);

        let mut lines = Vec::new();

        // First, split by explicit newlines
        let text = match str::from_utf8(text.strip_suffix(&[0xD]).unwrap_or(text)) {
            Ok(v) => v,
            Err(_) => {
                warn!("Failed to decode utf8 to str");
                return;
            }
        };
        for raw_line in text.lines() {
            let mut remaining = raw_line.trim();

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
        }

        info!("Printing");
        for line in lines.into_iter().rev() {
            self.print_line(line).await;
        }

        info!("Print complete");
        self.advance_paper(1).await;
    }

    async fn advance_paper(&mut self, lines: usize) {
        debug!("Advancing: {} lines", lines);
        for _ in 0..lines {
            self.printer.send_data(&[0x0A]).await; // LF
        }
    }

    async fn print_line(&mut self, line: &str) {
        debug!("Printing line: {}", line);

        self.printer.send_data(line.as_bytes()).await;
        self.printer.send_data(&[0x0A]).await; // LF
    }

    // fn set_inverse(&mut self, enable: bool) {
    //     uart.write_all(&[0x1D, b'B', if enable { 1 } else { 0 }])
    //         .unwrap();
    // }
    //
    async fn run(mut self) {
        loop {
            let data: MessageData = self.printer_rx.receive().await;
            info!("Received data: {}", data);
            self.print(data.as_bytes()).await;
        }
    }
}
