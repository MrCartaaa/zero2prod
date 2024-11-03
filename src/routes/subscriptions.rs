use actix_web::{web, HttpResponse};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub struct FormData {
    email: String,
    name: String,
}

pub async fn subscribe(_form: web::Form<FormData>, pool: web::Data<PgPool>) -> HttpResponse {
    let req_id = Uuid::new_v4();
    let request_span = tracing::info_span!(
        "request_id {} - Adding a new subscriber.",
        %req_id,
        subscriber_email = %_form.email,
        subscriber_name = %_form.name
    );
    let _request_span_guard = request_span.enter();

    let query_span = tracing::info_span!("Saving new subscriber details to the database.");

    match sqlx::query!(
        r#"
    INSERT INTO subscriptions (id, email, name, subscribed_at)
    VALUES ($1, $2, $3, $4)
    "#,
        Uuid::new_v4(),
        _form.email,
        _form.name,
        Utc::now()
    )
    .execute(pool.get_ref())
    .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            tracing::error!("request_id {} - Failed to execute query: {:?}", req_id, e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
