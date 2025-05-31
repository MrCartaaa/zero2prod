use crate::domain::{newsletters as newsletters_domain, SubscriberEmail};
use crate::email_client::EmailClient;
use crate::{configuration::Settings, startup::get_connection_pool};
use sqlx::{Executor, PgPool, Postgres, Transaction};
use std::time::Duration;
use tracing::{field::display, Span};
use uuid::Uuid;

pub enum ExecutionOutcome {
    TaskCompleted,
    EmptyQueue,
}

#[tracing::instrument(skip_all, fields(newsletter_id=tracing::field::Empty, subscriber_email=tracing::field::Empty,), err)]
pub async fn try_execute_task(
    pool: &PgPool,
    email_client: &EmailClient,
) -> Result<ExecutionOutcome, anyhow::Error> {
    let task = dequeue_task(pool).await?;
    if task.is_none() {
        return Ok(ExecutionOutcome::EmptyQueue);
    }
    let (trx, newsletter_id, email) = task.unwrap();
    Span::current()
        .record("newsletter_id", &display(newsletter_id))
        .record("subscriber_email", &display(&email));

    match SubscriberEmail::new(email.clone()) {
        Ok(email) => {
            let newsletter = newsletters_domain::get_newsletter(pool, newsletter_id).await?;
            if let Err(e) = email_client
                .send_email(
                    &email,
                    &newsletter.title,
                    &newsletter.html_content,
                    &newsletter.text_content,
                )
                .await
            {
                tracing::error!(
                    error.cause_chain = ?e,
                    error.message = %e,
                    "Failed to deliver newsletter to a confirmed subscriber. Skipping",
                );
            }
        }
        Err(e) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "Skipping a confirmed subscriber, their details are invalid.",
            );
        }
    }
    delete_task(trx, newsletter_id, &email).await?;
    Ok(ExecutionOutcome::TaskCompleted)
}

type PgTransaction = Transaction<'static, Postgres>;

#[tracing::instrument(skip_all)]
async fn dequeue_task(
    pool: &PgPool,
) -> Result<Option<(PgTransaction, Uuid, String)>, anyhow::Error> {
    let mut trx = pool.begin().await?;

    let r = sqlx::query!(
        r#"
        SELECT newsletter_id, subscriber_email
            FROM newsletter_delivery_queue
        FOR UPDATE
        SKIP LOCKED
        LIMIT 1
    "#
    )
    .fetch_optional(&mut *trx)
    .await?;
    if let Some(r) = r {
        Ok(Some((trx, r.newsletter_id, r.subscriber_email)))
    } else {
        Ok(None)
    }
}

#[tracing::instrument(skip_all)]
async fn delete_task(
    mut trx: PgTransaction,
    newsletter_id: Uuid,
    email: &str,
) -> Result<(), anyhow::Error> {
    let query = sqlx::query!(
        r#"
    DELETE FROM newsletter_delivery_queue WHERE newsletter_id = $1 AND subscriber_email = $2"#,
        newsletter_id,
        email
    );
    trx.execute(query).await?;
    trx.commit().await?;
    Ok(())
}

async fn worker_loop(pool: PgPool, email_client: EmailClient) -> Result<(), anyhow::Error> {
    loop {
        match try_execute_task(&pool, &email_client).await {
            Ok(ExecutionOutcome::EmptyQueue) => {
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Ok(ExecutionOutcome::TaskCompleted) => {}
        }
    }
}

pub async fn run_worker_until_stopped(config: Settings) -> Result<(), anyhow::Error> {
    let conn_pool = get_connection_pool(&config.database);

    worker_loop(conn_pool, config.email_client.client()).await
}
