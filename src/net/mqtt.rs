use alloc::{format, string::String};
use defmt::{debug, error, info};
use embassy_futures::select::select;
use embassy_net::{IpAddress, Stack, tcp::TcpSocket};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Timer};
use rust_mqtt::{
    client::{
        client,
        client_config::{ClientConfig, MqttVersion},
    },
    packet::v5::publish_packet::QualityOfService,
};

use crate::{glue::Rng, printer::PrinterWriter, shutdown::SHUTDOWN_WATCHER};

const MQTT_USER: &str = env!("MQTT_USER");
const MQTT_PASSWORD: &str = env!("MQTT_PASSWORD");

#[derive(Clone, Copy)]
pub enum Status {
    Up,
    ShuttingDown,
    Down,
}

impl Status {
    fn as_bytes(&self) -> &[u8] {
        match &self {
            Status::Up => "up".as_bytes(),
            Status::Down => "down".as_bytes(),
            Status::ShuttingDown => "shutting down".as_bytes(),
        }
    }
}

static STATUS_SIGNAL: Signal<CriticalSectionRawMutex, Status> = Signal::new();

pub async fn status_runner() {
    let mut shutdown_recv = match SHUTDOWN_WATCHER.receiver() {
        Some(recv) => recv,
        None => {
            panic!("Failed to retrieve shutdown recv")
        }
    };

    let mut current_status = Status::Up;

    loop {
        STATUS_SIGNAL.signal(current_status);
        if select(
            Timer::after(Duration::from_secs(10)),
            shutdown_recv.changed(),
        )
        .await
        .is_second()
        {
            STATUS_SIGNAL.signal(Status::ShuttingDown);
            current_status = Status::Down;
        }
    }
}

pub struct MQTTService {
    stack: Stack<'static>,
    rng: Rng,
    client_id: String,
    printer: PrinterWriter,
}

impl MQTTService {
    pub fn new(stack: Stack<'static>, rng: Rng, client_id: String, printer: PrinterWriter) -> Self {
        MQTTService {
            stack,
            rng,
            client_id,
            printer,
        }
    }

    pub async fn run(&self) {
        mqtt_runner(self.stack, self.rng, &self.client_id, &self.printer).await;
    }
}

async fn mqtt_runner(stack: Stack<'static>, rng: Rng, client_id: &str, printer: &PrinterWriter) {
    let mut rx_buffer = [0; 1024];
    let mut tx_buffer = [0; 1024];
    let mut recv_buffer = [0; 1024];
    let mut write_buffer = [0; 1024];

    'outer: loop {
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

        info!("Starting mqtt loop");
        let client_queue = format!("embedded/scribe/client/{client_id}");
        loop {
            match select(STATUS_SIGNAL.wait(), client.receive_message()).await {
                embassy_futures::select::Either::First(res) => {
                    if handle_status(&mut client, &client_queue, res)
                        .await
                        .is_err()
                    {
                        error!("Failed to handle Status");
                        continue 'outer;
                    }
                }
                embassy_futures::select::Either::Second(res) => match res {
                    Ok(msg) => handle_recieve(printer, msg.0, msg.1).await,
                    Err(e) => {
                        error!("MQTT Error in receive: {:?}", e);
                        continue 'outer;
                    }
                },
            }
        }
    }
}

async fn handle_status<'a>(
    client: &mut MqttClient<'a>,
    topic: &str,
    status: Status,
) -> Result<(), ()> {
    if client.send_ping().await.is_err() {
        return Err(());
    }

    if send_message(
        client,
        topic,
        status.as_bytes(),
        QualityOfService::QoS0,
        false,
    )
    .await
    .is_err()
    {
        Err(())
    } else {
        Ok(())
    }
}

async fn handle_recieve(printer: &PrinterWriter, topic: &str, payload: &[u8]) {
    let message = str::from_utf8(payload).unwrap_or("utf8 decode err");
    info!("Received message: {} - {}", topic, message);

    printer.print(message.into()).await;
}

type MqttClient<'a> = client::MqttClient<'a, TcpSocket<'a>, 5, Rng>;

async fn init_mqtt_client<'a>(
    stack: Stack<'static>,
    rng: Rng,
    client_id: &'a str,
    rx_buffer: &'a mut [u8],
    tx_buffer: &'a mut [u8],
    write_buffer: &'a mut [u8],
    recv_buffer: &'a mut [u8],
) -> Result<MqttClient<'a>, ()> {
    info!("initializing mqtt client");
    let mut socket = TcpSocket::new(stack, rx_buffer, tx_buffer);
    socket.set_timeout(Some(Duration::from_secs(30)));
    let ip = IpAddress::v4(192, 168, 1, 33);

    loop {
        match socket.connect((ip, 1883)).await {
            Ok(connection) => break connection,
            Err(_) => error!("Failed to connect to MQTT broker..."),
        }
        Timer::after(Duration::from_secs(5)).await;
    }

    let mut config = ClientConfig::new(MqttVersion::MQTTv5, rng);
    config.add_max_subscribe_qos(QualityOfService::QoS2);
    config.add_client_id(client_id);
    config.add_username(MQTT_USER);
    config.add_password(MQTT_PASSWORD);
    config.max_packet_size = recv_buffer.len() as u32;
    config.keep_alive = 10;

    let mut client = MqttClient::<'a>::new(
        socket,
        write_buffer,
        write_buffer.len(),
        recv_buffer,
        recv_buffer.len(),
        config,
    );

    if connect_to_broker(&mut client).await.is_err() {
        return Err(());
    }

    let producer_queue = "embedded/scribe/producer/#";
    info!("MQTT subscribing to: {}", producer_queue);
    if subscribe_to_topic(&mut client, producer_queue)
        .await
        .is_err()
    {
        return Err(());
    }
    Ok(client)
}

async fn subscribe_to_topic<'a>(client: &mut MqttClient<'a>, topic: &str) -> Result<(), ()> {
    let mut failure_count = 0;
    loop {
        match client.subscribe_to_topic(topic).await {
            Ok(()) => return Ok(()),
            Err(mqtt_error) => {
                error!("MQTT Error in subscribe: {:?}", mqtt_error);
                failure_count += 1;
            }
        }
        if failure_count > 5 {
            return Err(());
        }
        Timer::after(Duration::from_secs(5)).await;
    }
}

async fn connect_to_broker<'a>(client: &mut MqttClient<'a>) -> Result<(), ()> {
    let mut failure_count = 0;
    loop {
        match client.connect_to_broker().await {
            Ok(()) => return Ok(()),
            Err(mqtt_error) => {
                error!("MQTT Error in connect to broker: {:?}", mqtt_error);
                failure_count += 1;
            }
        }
        if failure_count > 5 {
            return Err(());
        }
        Timer::after(Duration::from_secs(5)).await;
    }
}

async fn send_message<'a>(
    client: &mut MqttClient<'a>,
    topic: &str,
    message: &[u8],
    qos: QualityOfService,
    retain: bool,
) -> Result<(), ()> {
    match client.send_message(topic, message, qos, retain).await {
        Ok(()) => {
            debug!("sent message");
            Ok(())
        }
        Err(mqtt_error) => {
            error!("MQTT Error in send: {:?}", mqtt_error);
            Err(())
        }
    }
}