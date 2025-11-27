#![no_std]
#![feature(impl_trait_in_assoc_type)]

pub extern crate alloc;

pub mod glue;
mod net;
mod power;
mod printer;

pub mod prelude;
pub use crate::net::mqtt::start_mqtt_client;
pub use crate::net::web::start_web_host;
pub use crate::net::wifi::start_wifi;
pub use crate::power::start_power_monitor;
pub use crate::printer::start_printer;

#[macro_export]
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}
