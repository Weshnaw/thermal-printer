use defmt::error;
use embassy_net::{
    dns::{self, DnsQueryType},
    IpAddress, Stack,
};

pub async fn lookup(url: &str, stack: Stack<'static>) -> Result<IpAddress, dns::Error> {
    let mut count = 0;
    loop {
        match stack.dns_query(url, DnsQueryType::A).await.map(|a| a[0]) {
            Ok(address) => {
                break Result::Ok(address);
            }
            Err(e) => {
                error!("DNS lookup error #{}: {:?}", count, e);
                if count >= 5 {
                    break Result::Err(e);
                }
            }
        };
        count += 1;
    }
}
