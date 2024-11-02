use std::net::TcpListener;
use zero2prod::startup::run;
use zero2prod::configuration::get_configuration;


#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let config = get_configuration().expect("Failed to read config file");
    let address = format!("127.0.0.1:{}", {config.application_port});
    let listener = TcpListener::bind(address)
        .expect("Failed to bind random port");
    run(listener)?.await
}
