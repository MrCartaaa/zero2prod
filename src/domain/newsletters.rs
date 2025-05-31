use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres, Transaction};
use uuid::Uuid;

pub struct NewsLetter {
    pub newsletter_id: String,
    pub title: String,
    pub text_content: String,
    pub html_content: String,
    pub published_at: DateTime<Utc>,
}

#[tracing::instrument(skip_all)]
pub async fn get_newsletter(
    pool: &PgPool,
    newsletter_id: Uuid,
) -> Result<NewsLetter, anyhow::Error> {
    let issue = sqlx::query_as!(
        NewsLetter,
        r#"
    SELECT * FROM newsletters WHERE newsletter_id = $1"#,
        newsletter_id
    )
    .fetch_one(pool)
    .await?;
    Ok(issue)
}

#[tracing::instrument(skip_all)]
pub async fn insert_newsletter(
    trx: &mut Transaction<'_, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let newsletter_id = Uuid::new_v4();

    let query = sqlx::query!(
        r#"
        INSERT INTO newsletters (
            newsletter_id,
            title,
            text_content,
            html_content,
            published_at
        )
        VALUES ($1, $2, $3, $4, now())
    "#,
        newsletter_id,
        title,
        text_content,
        html_content
    );
    trx.execute(query).await?;
    Ok(newsletter_id)
}
