use std::fmt::{Debug, Display};
use tokio::task::JoinError;
use zero2prod::configuration::get_configuration;
use zero2prod::newsletter_delivery_worker::run_worker_until_stopped;
use zero2prod::startup::Application;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let config = get_configuration().expect("Failed to read config file");

    let application_task = tokio::spawn(
        Application::build(config.clone())
            .await?
            .run_until_stopped(),
    );
    let worker_task = tokio::spawn(run_worker_until_stopped(config));

    tokio::select! {
        o=application_task => report_exit("API", o),
        o=worker_task => report_exit("Backgroun Worker", o),
    }
    Ok(())
}

fn report_exit(task_name: &str, outcome: Result<Result<(), impl Debug + Display>, JoinError>) {
    match outcome {
        Ok(Ok(())) => {
            tracing::info!("{} has exited", task_name)
        }
        Ok(Err(e)) => {
            tracing::error!(error.cause_chain=?e,error.message=%e, "{} failed", task_name)
        }
        Err(e) => {
            tracing::error!(
                error.cause_chain = ?e,
                    error.message = %e,
                    "{} task failed to complete",
                    task_name,
            )
        }
    }
}
