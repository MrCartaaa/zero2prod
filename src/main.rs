use sqlx::postgres::PgPoolOptions;
use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let config = get_configuration().expect("Failed to read config file");

    let connection_pool = PgPoolOptions::new().connect_lazy_with(config.database.connect_options());

    let address = format!("127.0.0.1:{}", { config.application.port });

    let listener = TcpListener::bind(address)?;

    let _ = run(listener, connection_pool)?.await;
    Ok(())
}
