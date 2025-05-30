use crate::authentication::{Credentials, UserId};

use crate::domain::get_username;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::idempotency::{save_response, try_processing, IdempotencyKey, NextAction};
use crate::routes::error_chain_fmt;
use actix_web::http::header::{HeaderMap, HeaderValue};
use actix_web::http::{header, StatusCode};
use actix_web::{web, HttpResponse, ResponseError};
use anyhow::Context;
use base64::Engine;
use secrecy::SecretBox;
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize, Debug)]
pub struct BodyData {
    title: String,
    content: Content,
    idempotency_key: String,
}

#[derive(Deserialize, Debug)]
pub struct Content {
    html: String,
    text: String,
}

#[tracing::instrument(name="Publish newsletter issue.", skip(pool, email_client, body), fields(username=tracing::field::Empty, user_id=tracing::field::Empty))]
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, PublishError> {
    let username = get_username(*user_id.clone().into_inner(), &pool).await?;
    tracing::Span::current().record("username", tracing::field::display(&username));
    tracing::Span::current().record(
        "user_id",
        tracing::field::display(*user_id.clone().into_inner()),
    );
    let idempotency_key: &IdempotencyKey = &body
        .idempotency_key
        .to_owned()
        .try_into()
        .map_err(|e: anyhow::Error| PublishError::ValidationError(format!("{}", e)))?;
    let trx = match try_processing(&pool, idempotency_key, *user_id.clone().into_inner()).await? {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(_) => {
            return Err(PublishError::ValidationError(
                "The newsletter has already been posted.".to_string(),
            ));
        }
    };

    let subscribers = get_confirmed_subscribers(&pool).await?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &body.title,
                        &body.content.html,
                        &body.content.text,
                    )
                    .await
                    .with_context(|| format!("Failed to send email to {}", subscriber.email))?;
            }
            Err(error) => {
                tracing::warn!(error.cause_chain = ?error, "Skipping a confirmed subscriber, their details are invalid.");
            }
        }
    }
    let response = save_response(
        trx,
        &idempotency_key,
        **user_id,
        HttpResponse::Ok().finish(),
    )
    .await?;

    Ok(response)
}

// removing basic auth -- keeping for reference.
#[allow(dead_code)]
fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header is missing.")?
        .to_str()
        .context("The 'Authorization' header value is valid UTF8.")?;
    let base64encoded_segment = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme was not 'Basic'.")?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(base64encoded_segment)
        .context("Failed to decode the authorization scheme.")?;
    let decoded_creds =
        String::from_utf8(decoded).context("The authorization scheme was not valid UTF8.")?;

    let mut creds = decoded_creds.splitn(2, ":");
    let username = creds
        .next()
        .ok_or_else(|| anyhow::anyhow!("Missing username in authorization scheme."))?
        .to_string();
    let password = creds
        .next()
        .ok_or_else(|| anyhow::anyhow!("Missing password in authorization scheme."))?
        .to_string();

    Ok(Credentials {
        username,
        password: SecretBox::new(Box::from(password)),
    })
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
    #[error("{0}")]
    ValidationError(String),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse {
        match self {
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            PublishError::AuthError(_) => {
                let mut resp = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_val = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
                resp.headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_val);
                resp
            }
            PublishError::ValidationError(err) => HttpResponse::BadRequest().body(err.clone()),
        }
    }
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(skip(pool), name = "Get Confirmed Subscribers")]
async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let rows = sqlx::query!(r#"SELECT email from subscriptions WHERE status = 'confirmed';"#)
        .fetch_all(pool)
        .await?;

    let confirmed_subscribers = rows
        .into_iter()
        .map(|r| match SubscriberEmail::new(r.email) {
            Ok(email) => Ok(ConfirmedSubscriber { email }),
            Err(error) => Err(anyhow::anyhow!(error)),
        })
        .collect();

    Ok(confirmed_subscribers)
}
