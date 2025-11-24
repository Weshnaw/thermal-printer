use alloc::sync::Arc;
use defmt::info;
use embassy_net::Stack;
use embassy_time::Duration;
use picoserve::{
    AppRouter, AppWithStateBuilder,
    extract::State,
    response::{File, IntoResponse},
    routing,
};

use crate::printer::PrinterWriter;

pub struct WebService {
    stack: Stack<'static>,
    router: &'static AppRouter<Application>,
    config: &'static picoserve::Config<Duration>,
    state: AppState,
}

impl WebService {
    pub async fn new(stack: Stack<'static>, printer: PrinterWriter) -> WebService {
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

    pub async fn run(
        &self,
        id: usize,
        rx_buffer: &mut [u8],
        tx_buffer: &mut [u8],
        http_buffer: &mut [u8],
    ) -> ! {
        let port = 80;

        picoserve::Server::new(
            &self.router.shared().with_state(&self.state),
            self.config,
            http_buffer,
        )
        .listen_and_serve(id, self.stack, port, rx_buffer, tx_buffer)
        .await
        .into_never()
    }
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

#[derive(Clone)]
struct AppState {
    printer: PrinterWriter,
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
