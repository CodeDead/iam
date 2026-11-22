use actix_web::{App, HttpServer, web};
use tracing::info;
use tracing_actix_web::TracingLogger;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use auth_backend::{
    app::{AppState, build_cors, configure_app_services},
    config::Settings,
    middleware::RequestIdAndSecurityHeaders,
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    init_tracing();

    let settings = match Settings::load() {
        Ok(s) => s,
        Err(err) => {
            tracing::error!(error = %err, "Failed to load application configuration");
            return Err(std::io::Error::other(err.to_string()));
        }
    };

    let bind_addr = format!("{}:{}", settings.server.host, settings.server.port);
    let state = match AppState::from_mongodb(settings).await {
        Ok(s) => s,
        Err(err) => {
            tracing::error!(error = %err, "Failed to initialize application state");
            return Err(std::io::Error::other(err.to_string()));
        }
    };

    let shared_state = web::Data::new(state);
    info!(
        address = %bind_addr,
        environment = %shared_state.settings.app.environment,
        trust_proxy_headers = shared_state.settings.rate_limit.trust_proxy_headers,
        rate_limit_enabled = shared_state.settings.rate_limit.enabled,
        swagger_enabled = shared_state.settings.is_swagger_enabled(),
        "Starting HTTP server"
    );

    HttpServer::new(move || {
        let cors = build_cors(&shared_state.settings);
        App::new()
            .wrap(TracingLogger::default())
            .wrap(RequestIdAndSecurityHeaders)
            .wrap(cors)
            .configure(|cfg| configure_app_services(cfg, shared_state.clone()))
    })
    .bind(&bind_addr)?
    .run()
    .await
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,auth_backend=debug,actix_web=info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().compact())
        .init();
}
