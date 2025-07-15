use core::marker::PhantomData;

use alloc::sync::Arc;
use defmt::info;
use embassy_net::Stack;
use embassy_sync::{blocking_mutex::raw::RawMutex, channel::Sender};
use embassy_time::Duration;
use esp_alloc as _;
use picoserve::{
    extract::State,
    response::{File, IntoResponse},
    routing, AppRouter, AppWithStateBuilder,
};

pub type MessageData = Arc<str>;

// TODO; move to data module
pub trait DataSender<T> {
    fn send(&self, message: T) -> impl core::future::Future<Output = ()>;
}

impl<'a, M, T, const N: usize> DataSender<T> for Sender<'a, M, T, N>
where
    M: RawMutex,
{
    async fn send(&self, message: T) {
        self.send(message).await
    }
}

pub async fn web_task_runner<S: DataSender<MessageData> + Clone>(
    id: usize,
    stack: Stack<'static>,
    router: &'static AppRouter<Application<S>>,
    config: &'static picoserve::Config<Duration>,
    state: AppState<S>,
) -> ! {
    let port = 80;
    let mut tcp_rx_buffer = [0; 1024];
    let mut tcp_tx_buffer = [0; 1024];
    let mut http_buffer = [0; 2048];

    picoserve::listen_and_serve_with_state(
        id,
        router,
        config,
        stack,
        port,
        &mut tcp_rx_buffer,
        &mut tcp_tx_buffer,
        &mut http_buffer,
        &state,
    )
    .await
}

#[derive(Clone)]
pub struct AppState<S: DataSender<MessageData>> {
    pub sender: S,
}

pub struct Application<S>(pub PhantomData<S>);

impl<S> Application<S> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

static INDEX_PAGE: &str = include_str!("index.html");
impl<S: DataSender<MessageData> + Clone> AppWithStateBuilder for Application<S> {
    type PathRouter = impl routing::PathRouter<AppState<S>>;
    type State = AppState<S>;

    fn build_app(self) -> picoserve::Router<Self::PathRouter, Self::State> {
        picoserve::Router::new().route(
            "/",
            routing::get_service(File::html(INDEX_PAGE)).post(post_handler::<S>),
        )
    }
}

#[derive(serde::Deserialize)]
struct SubmitData {
    message: MessageData,
}

async fn post_handler<S: DataSender<MessageData> + Clone>(
    State(state): picoserve::extract::State<AppState<S>>,
    data: picoserve::extract::Form<SubmitData>,
) -> impl IntoResponse {
    info!("Received message: {}", data.message.as_ref());

    state.sender.send(data.message.clone()).await;
}
