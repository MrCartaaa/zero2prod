use crate::routes::error_chain_fmt;
use crate::session_state::TypedSession;
use crate::utils::e500;
use actix_web::http::header::HeaderValue;
use actix_web::http::{header, StatusCode};
use actix_web::{web, HttpResponse, ResponseError};
use anyhow::Context;
use secrecy::{ExposeSecretMut, SecretString};

#[derive(serde::Deserialize)]
pub struct FormData {
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
                return HttpResponse::BadRequest().body(err.clone())
            }
        }
    }
}

pub async fn change_password(
    mut form: web::Form<FormData>,
    session: TypedSession,
) -> Result<HttpResponse, ChangePasswordError> {
    let user_id = session.get_user_id().context("unable to identify user")?;
    println!("{:?}", user_id);
    if user_id.is_none() {
        return Err(ChangePasswordError::Unauthorized());
    };
    if form.0.new_password.expose_secret_mut() != form.0.new_password_check.expose_secret_mut() {
        return Err(ChangePasswordError::ValidationError(
            "new password must be confirmed.".to_string(),
        ));
    };

    Ok(HttpResponse::Ok().finish())
}
