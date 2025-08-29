use alloc::{boxed::Box, sync::Arc};
use defmt::info;
use embassy_net::Stack;
use embassy_time::Duration;
use esp_alloc as _;
use picoserve::{
    extract::State,
    response::{File, IntoResponse},
    routing, AppRouter, AppWithStateBuilder,
};

use crate::printer::ThermalPrinter;

pub struct WebService {
    stack: Stack<'static>,
    router: &'static AppRouter<Application>,
    config: &'static picoserve::Config<Duration>,
    state: AppState,
}

impl WebService {
    pub async fn new(stack: Stack<'static>, printer: ThermalPrinter) -> WebService {
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

        Self {
            stack,
            router,
            config,
            state: AppState { printer },
        }
    }

    pub async fn run(&self, id: usize) {
        let port = 80;
        
        // force the buffers into static memory
        let tcp_rx_buffer = Box::leak(Box::new([0; 1024]));
        let tcp_tx_buffer = Box::leak(Box::new([0; 1024]));
        let http_buffer = Box::leak(Box::new([0; 2048]));

        picoserve::listen_and_serve_with_state(
            id,
            self.router,
            self.config,
            self.stack,
            port,
            tcp_rx_buffer,
            tcp_tx_buffer,
            http_buffer,
            &self.state,
        )
        .await
    }
}

#[derive(Clone)]
struct AppState {
    printer: ThermalPrinter,
}

struct Application;

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

#[derive(serde::Deserialize)]
struct SubmitData {
    message: Arc<str>,
}

async fn post_handler(
    State(state): picoserve::extract::State<AppState>,
    data: picoserve::extract::Form<SubmitData>,
) -> impl IntoResponse {
    info!("Received message: {}", data.message.as_ref());

    state.printer.print(data.message.clone()).await;
}
