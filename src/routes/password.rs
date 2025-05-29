use crate::authentication as auth;
use crate::domain::get_username;
use crate::routes::error_chain_fmt;
use crate::session_state::TypedSession;
use actix_web::http::header::HeaderValue;
use actix_web::http::{header, StatusCode};
use actix_web::{web, HttpResponse, ResponseError};
use anyhow::Context;
use secrecy::{ExposeSecretMut, SecretString};
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct PasswordFormData {
    current_password: SecretString,
    new_password: SecretString,
    new_password_check: SecretString,
}

#[derive(thiserror::Error)]
pub enum ChangePasswordError {
    #[error("{0}")]
    ValidationError(String),

    #[error("")]
    Unauthorized(),

    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ChangePasswordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ChangePasswordError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ChangePasswordError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            ChangePasswordError::Unauthorized() => {
                let mut resp = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_val = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
                resp.headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_val);
                resp
            }
            ChangePasswordError::ValidationError(err) => {
                HttpResponse::BadRequest().body(err.clone())
            }
        }
    }
}

pub async fn change_password(
    mut form: web::Form<PasswordFormData>,
    session: TypedSession,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ChangePasswordError> {
    let user_id = session.get_user_id().context("unable to identify user")?;
    if user_id.is_none() {
        return Err(ChangePasswordError::Unauthorized());
    };
    let user_id = user_id.unwrap();

    let new_password = form.0.new_password.expose_secret_mut();

    if new_password != form.0.new_password_check.expose_secret_mut() {
        return Err(ChangePasswordError::ValidationError(
            "new password must be confirmed.".to_string(),
        ));
    };

    if new_password.chars().count() < 12 || new_password.chars().count() > 129 {
        return Err(ChangePasswordError::ValidationError(
            "new password must meet requirements.".to_string(),
        ));
    }
    let username = get_username(user_id, &pool).await?;

    let credentials = auth::Credentials {
        username,
        password: form.0.current_password,
    };
    if let Err(e) = auth::validate_credentials(credentials, &pool).await {
        return match e {
            auth::AuthError::InvalidCredentials(_) => Err(ChangePasswordError::Unauthorized()),
            auth::AuthError::UnexpectedError(_) => {
                Err(ChangePasswordError::UnexpectedError(e.into()))
            }
        };
    }

    auth::change_password(user_id, form.0.new_password, &pool).await?;

    Ok(HttpResponse::Ok().finish())
}
