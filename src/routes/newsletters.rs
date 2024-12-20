use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::routes::error_chain_fmt;
use crate::telemetry::spawn_blocking_with_tracing;
use actix_web::http::header::{HeaderMap, HeaderValue};
use actix_web::http::{header, StatusCode};
use actix_web::{web, HttpRequest, HttpResponse, ResponseError};
use anyhow::{anyhow, Context};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use base64::Engine;
use secrecy::ExposeSecret;
use secrecy::SecretBox;
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(Deserialize)]
pub struct Content {
    html: String,
    text: String,
}

#[tracing::instrument(name="Publish newsletter issue.", skip(pool, email_client, body, request), fields(username=tracing::field::Empty, user_id=tracing::field::Empty))]
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    request: HttpRequest,
) -> Result<HttpResponse, PublishError> {
    let credentials = basic_authentication(request.headers()).map_err(PublishError::AuthError)?;

    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));
    let user_id = validate_credentials(credentials, &pool).await?;
    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));

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
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "Validate Credentials", skip(credentials, pool))]
async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, PublishError> {
    let mut user_id = None;
    let mut expected_password_hash = "$argon2id$v=19$m=15000,t=2,p=1gZiV/M1gPc22ELAH/Jh1H2$CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno".to_string();

    if let Some((stored_user_id, stored_password_hash)) =
        get_stored_credentials(&credentials.username, &pool)
            .await
            .map_err(PublishError::UnexpectedError)?
    {
        user_id = Some(stored_user_id);
        expected_password_hash = stored_password_hash;
    };

    spawn_blocking_with_tracing(move || {
        tracing::info_span!("Verify password hash")
            .in_scope(|| verify_password_hash(expected_password_hash, credentials.password))
    })
    .await
    .context("Failed to spawn blocking task")
    .map_err(PublishError::UnexpectedError)??;

    user_id.ok_or_else(|| PublishError::AuthError(anyhow!("Unknown username.")))
}

#[tracing::instrument(name = "Get stored credentials.", skip(username, pool))]
async fn get_stored_credentials(
    username: &str,
    pool: &PgPool,
) -> Result<Option<(uuid::Uuid, String)>, anyhow::Error> {
    let row: Option<_> = sqlx::query!(
        r#"
        SELECT user_id, password_hash
        FROM users
        WHERE username = $1
        "#,
        username
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to validate auth credentials.")?
    .map(|row| (row.user_id, row.password_hash));

    Ok(row)
}

#[tracing::instrument(
    name = "Verify password hash",
    skip(expected_password_hash, password_candidate)
)]
fn verify_password_hash(
    expected_password_hash: String,
    password_candidate: SecretBox<String>,
) -> Result<(), PublishError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.as_str())
        .context("Failed to parse hash in PHC string format.")
        .map_err(PublishError::UnexpectedError)?;
    Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        )
        .context("Invalid Password")
        .map_err(PublishError::AuthError)
}

struct Credentials {
    username: String,
    password: SecretBox<String>,
}

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
