use defmt::info;
use embassy_net::Stack;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Sender};
use embassy_time::Duration;
use esp_alloc as _;
use picoserve::{
    extract::State,
    response::{File, IntoResponse},
    routing, AppRouter, AppWithStateBuilder, Router,
};

pub struct Application;

#[derive(Clone)]
pub struct AppState {
    pub sender: Sender<'static, CriticalSectionRawMutex, heapless::String<MAX_BODY_LEN>, 8>,
}

static INDEX_PAGE: &str = include_str!("index.html");
impl AppWithStateBuilder for Application {
    type PathRouter = impl routing::PathRouter<AppState>;
    type State = AppState;

    fn build_app(self) -> picoserve::Router<Self::PathRouter, Self::State> {
        picoserve::Router::new().route(
            "/",
            routing::get_service(File::html(INDEX_PAGE)).post(post_handler),
        )
    }
}
/// Max body size we accept
pub const MAX_BODY_LEN: usize = 512;

#[derive(serde::Deserialize)]
struct SubmitData {
    message: heapless::String<MAX_BODY_LEN>,
}

async fn post_handler(
    State(state): picoserve::extract::State<AppState>,
    data: picoserve::extract::Form<SubmitData>,
) -> impl IntoResponse {
    info!("Received message: {}", data.message);

    state.sender.send(data.message.clone()).await;
}

pub const WEB_TASK_POOL_SIZE: usize = 2;

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
pub async fn web_task(
    id: usize,
    stack: Stack<'static>,
    router: &'static AppRouter<Application>,
    config: &'static picoserve::Config<Duration>,
    state: AppState,
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

pub struct WebApp {
    pub router: &'static Router<<Application as AppWithStateBuilder>::PathRouter, AppState>,
    pub config: &'static picoserve::Config<Duration>,
}

impl Default for WebApp {
    fn default() -> Self {
        let router = picoserve::make_static!(AppRouter<Application>, Application.build_app());

        let config = picoserve::make_static!(
            picoserve::Config<Duration>,
            picoserve::Config::new(picoserve::Timeouts {
                start_read_request: Some(Duration::from_secs(5)),
                read_request: Some(Duration::from_secs(1)),
                write: Some(Duration::from_secs(1)),
                persistent_start_read_request: Some(Duration::from_secs(5))
            })
            .keep_connection_alive()
        );

        Self { router, config }
    }
}
