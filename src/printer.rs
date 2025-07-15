// TODO; make better

use defmt::info;

pub async fn initialize_printer<T: embedded_io::Write>(uart: &mut T) {
    uart.write_all(&[0x1B, b'@']).unwrap(); // ESC @
    embassy_time::Timer::after_millis(50).await;

    uart.write_all(&[0x1B, b'7', 15, 150, 250]).unwrap(); // print density
    uart.write_all(&[0x1B, b'{', 0x01]).unwrap(); // 180° rotation

    defmt::info!("Printer initialized");
}

pub fn set_inverse<T: embedded_io::Write>(uart: &mut T, enable: bool) {
    uart.write_all(&[0x1D, b'B', if enable { 1 } else { 0 }])
        .unwrap();
}

pub fn advance_paper<T: embedded_io::Write>(uart: &mut T, lines: usize) {
    for _ in 0..lines {
        uart.write_all(&[0x0A]).unwrap(); // LF
    }
}

pub fn print_line<T: embedded_io::Write>(uart: &mut T, line: &str) {
    uart.write_all(line.as_bytes()).unwrap();
    uart.write_all(&[0x0A]).unwrap(); // LF
}

pub fn print_wrapped_upside_down<T: embedded_io::Write>(
    uart: &mut T,
    text: &str,
    max_chars_per_line: usize,
) {
    info!("printing: {}", text);
    let mut lines = heapless::Vec::<heapless::String<64>, 100>::new();

    let mut remaining = text.trim();
    while !remaining.is_empty() {
        let take_len = core::cmp::min(max_chars_per_line, remaining.len());
        let slice = &remaining[..take_len];

        let break_point = slice.rfind(' ').unwrap_or(take_len);
        let (line, rest) = remaining.split_at(break_point);

        let mut line_buf = heapless::String::new();
        line_buf.push_str(line.trim()).unwrap();
        lines.push(line_buf).unwrap();

        remaining = rest.trim_start();
    }

    for line in lines.iter().rev() {
        print_line(uart, line);
    }
}
