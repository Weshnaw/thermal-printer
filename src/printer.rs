use alloc::sync::Arc;
use defmt::{debug, info};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};

type MessageData = Arc<str>;
type PrinterChannel = Channel<CriticalSectionRawMutex, MessageData, 8>;
type PrinterSender = Sender<'static, CriticalSectionRawMutex, MessageData, 8>;
type PrinterReceiver = Receiver<'static, CriticalSectionRawMutex, MessageData, 8>;

static PRINTER_CHANNEL: PrinterChannel = Channel::new();
static MAX_CHARACTERS_PER_LINE: usize = 10;

pub async fn new<T: embedded_io::Write>(writer: T) -> (ThermalPrinterService<T>, ThermalPrinter) {
    let mut printer = ThermalPrinterService::new(writer).await;
    printer.print("Test Print, extra lines 12345678901234567890");

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

    pub async fn print(&self, buf: &str) {
        self.printer_tx.send(buf.into()).await;
    }
}

impl Default for ThermalPrinter {
    fn default() -> Self {
        Self::new()
    }
}

// TODO; setup a configuration that lets this be changed dynamically via mqtt
pub struct ThermalPrinterService<T: embedded_io::Write> {
    serial_writer: T,
    printer_rx: PrinterReceiver,
}

impl<T: embedded_io::Write> ThermalPrinterService<T> {
    async fn new(mut serial_writer: T) -> Self {
        serial_writer.write_all(&[0x1B, b'@']).unwrap(); // ESC @
        embassy_time::Timer::after_millis(50).await;

        serial_writer
            .write_all(&[0x1B, b'7', 15, 150, 250])
            .unwrap(); // print density
        serial_writer.write_all(&[0x1B, b'{', 0x01]).unwrap(); // 180° rotation

        let printer_rx = PRINTER_CHANNEL.receiver();

        Self {
            serial_writer,
            printer_rx,
        }
    }

    fn print(&mut self, text: &str) {
        info!("Printing: {}", text);

        let mut remaining = text.trim_ascii_end();
        while !remaining.is_empty() {
            // Limit to MAX_CHARACTERS from the end
            let take_len = core::cmp::min(MAX_CHARACTERS_PER_LINE, remaining.len());
            let start_idx = remaining.len() - take_len;
            let slice = &remaining[start_idx..];

            // Find a space to break at, scanning from the front of the slice
            let break_point = slice.find(' ').unwrap_or(0);

            let split_idx = if break_point == 0 {
                // No space found, or first character is space — just break at slice start
                start_idx
            } else {
                start_idx + break_point
            };

            // Split remaining text
            let (head, tail) = remaining.split_at(split_idx);
            let line = tail.trim();

            self.print_line(line);

            remaining = head.trim_end();
        }

        self.advance_paper(2);
    }

    fn advance_paper(&mut self, lines: usize) {
        debug!("Advancing: {} lines", lines);
        for _ in 0..lines {
            self.serial_writer.write_all(&[0x0A]).unwrap(); // LF
        }
    }

    fn print_line(&mut self, line: &str) {
        debug!("Printing line: {}", line);

        self.serial_writer.write_all(line.as_bytes()).unwrap();
        self.serial_writer.write_all(&[0x0A]).unwrap(); // LF
    }

    // fn set_inverse(&mut self, enable: bool) {
    //     uart.write_all(&[0x1D, b'B', if enable { 1 } else { 0 }])
    //         .unwrap();
    // }
    //
    pub async fn run(mut self) {
        loop {
            let data = self.printer_rx.receive().await;
            self.print(&data);
        }
    }
}
