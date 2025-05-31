use sqlx::{Executor, Postgres, Transaction};
use uuid::Uuid;

pub async fn queue_delivery_task(
    trx: &mut Transaction<'_, Postgres>,
    newsletter_id: Uuid,
) -> Result<(), sqlx::Error> {
    let query = sqlx::query!(
        r#"
    INSERT INTO newsletter_delivery_queue (
        newsletter_id,
        subscriber_email
    )
    SELECT $1, email
        FROM subscriptions
    WHERE status = 'confirmed'"#,
        newsletter_id,
    );
    trx.execute(query).await?;
    Ok(())
}
