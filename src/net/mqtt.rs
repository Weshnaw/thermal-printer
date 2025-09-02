use alloc::{format, string::String};
use defmt::{error, info};
use embassy_net::{tcp::TcpSocket, IpAddress, Stack};
use embassy_time::{Duration, Timer};
use esp_hal::rng::Rng;
use rust_mqtt::{
    client::{
        client::MqttClient,
        client_config::{ClientConfig, MqttVersion},
    },
    packet::v5::{publish_packet::QualityOfService, reason_codes::ReasonCode},
};

const MQTT_USER: &str = env!("MQTT_USER");
const MQTT_PASSWORD: &str = env!("MQTT_PASSWORD");

pub struct MQTTService {
    stack: Stack<'static>,
    rng: Rng,
    client_id: String,
}

impl MQTTService {
    pub fn new(stack: Stack<'static>, rng: Rng, client_id: String) -> Self {
        MQTTService {
            stack,
            rng,
            client_id,
        }
    }

    pub async fn run(&self) {
        mqtt_runner(self.stack, self.rng, &self.client_id).await;
    }
}

async fn init_mqtt_client<'a>(
    stack: Stack<'static>,
    rng: Rng,
    client_id: &'a str,
    rx_buffer: &'a mut [u8],
    tx_buffer: &'a mut [u8],
    write_buffer: &'a mut [u8],
    recv_buffer: &'a mut [u8],
) -> Result<MqttClient<'a, TcpSocket<'a>, 5, Rng>, ()> {
    info!("initializing mqtt client");
    let mut socket = TcpSocket::new(stack, rx_buffer, tx_buffer);
    socket.set_timeout(Some(Duration::from_secs(10)));
    let ip = IpAddress::v4(192, 168, 1, 33); // TODO; make configurable

    loop {
        match socket.connect((ip, 1883)).await {
            Ok(connection) => break connection,
            Err(_) => error!("Failed to connect to MQTT broker..."),
        }
        Timer::after(Duration::from_secs(5)).await;
    }

    let mut config = ClientConfig::new(MqttVersion::MQTTv5, rng);
    config.add_max_subscribe_qos(QualityOfService::QoS0);
    config.add_client_id(client_id);
    config.add_username(MQTT_USER);
    config.add_password(MQTT_PASSWORD);
    config.max_packet_size = 1000;

    let mut client =
        MqttClient::<_, 5, _>::new(socket, write_buffer, 1024, recv_buffer, 1024, config);

    let mut failure_count = 0;
    loop {
        match client.connect_to_broker().await {
            Ok(()) => break,
            Err(mqtt_error) => match mqtt_error {
                ReasonCode::NetworkError => {
                    error!("MQTT Network Error");
                    failure_count += 1;
                }
                _ => {
                    error!("Other MQTT Error: {:?}", mqtt_error);
                    failure_count += 1;
                }
            },
        }
        if failure_count > 5 {
            return Err(());
        }
        Timer::after(Duration::from_secs(5)).await;
    }

    Ok(client)
}

async fn mqtt_runner(stack: Stack<'static>, rng: Rng, client_id: &str) {
    let mut rx_buffer = [0; 1024];
    let mut tx_buffer = [0; 1024];
    let mut recv_buffer = [0; 1024];
    let mut write_buffer = [0; 1024];

    let mut client = loop {
        if let Ok(client) = init_mqtt_client(
            stack,
            rng,
            client_id,
            &mut rx_buffer,
            &mut tx_buffer,
            &mut write_buffer,
            &mut recv_buffer,
        )
        .await
        {
            break client;
        }
    };

    // TODO; I would prefere to have two loops here one for receiving and one for sending
    let client_queue = format!("embedded/scribe/client/{client_id}");
    let mut failure_count = 0;
    loop {
        match client
            .send_message(&client_queue, "up".as_bytes(), QualityOfService::QoS0, true)
            .await
        {
            Ok(()) => {
                info!("sent message");
            }
            Err(mqtt_error) => match mqtt_error {
                ReasonCode::NetworkError => {
                    error!("MQTT Network Error");
                    failure_count += 1;
                }
                _ => {
                    error!("Other MQTT Error: {:?}", mqtt_error);
                    failure_count += 1;
                }
            },
        }
        if failure_count > 5 {
            drop(client);
            client = loop {
                if let Ok(client) = init_mqtt_client(
                    stack,
                    rng,
                    client_id,
                    &mut rx_buffer,
                    &mut tx_buffer,
                    &mut write_buffer,
                    &mut recv_buffer,
                )
                .await
                {
                    break client;
                }
            };
            failure_count = 0;
        }
        Timer::after(Duration::from_secs(5)).await;
    }
}

// TODO:
//  - receive messages
//  - handle configuration messages
