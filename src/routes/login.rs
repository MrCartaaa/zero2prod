use crate::authentication::{validate_credentials, AuthError, Credentials};
use crate::cloneable_auth_token::SecretAuthToken;
use crate::routes::error_chain_fmt;
use crate::session_state::TypedSession;
use crate::startup::HmacSecret;
use actix_web::error::InternalError;
use actix_web::http::StatusCode;
use actix_web::{web, HttpResponse};
use ring::hmac;
use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct LoginFormData {
    username: String,
    password: SecretString,
}

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Authentication Failed.")]
    AuthError(#[source] anyhow::Error),
    #[error("Something went wrong.")]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

fn build_err_resp(secret: &SecretAuthToken, err: LoginError) -> InternalError<LoginError> {
    let message = format!("{}", urlencoding::encode(err.to_string().as_str()));

    let key_value: &[u8] = secret.expose_secret().token.as_bytes();

    let key = hmac::Key::new(hmac::HMAC_SHA256, key_value);

    let sig = hmac::sign(&key, message.as_bytes());

    let status_code = match err {
        LoginError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        LoginError::AuthError(_) => StatusCode::UNAUTHORIZED,
    };
    let http_resp = HttpResponse::build(status_code).body(format!("{:?}", sig.as_ref()));

    InternalError::from_response(err, http_resp)
}

#[tracing::instrument(skip(form, pool, secret, session), fields(username=tracing::field::Empty, user_id=tracing::field::Empty))]
pub async fn login(
    form: web::Form<LoginFormData>,
    pool: web::Data<PgPool>,
    secret: web::Data<HmacSecret>,
    session: TypedSession,
) -> Result<HttpResponse, InternalError<LoginError>> {
    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password,
    };

    tracing::Span::current().record("username", tracing::field::display(&credentials.username));

    match validate_credentials(credentials, &pool).await {
        Ok(user_id) => {
            tracing::Span::current().record("user_id", tracing::field::display(&user_id));
            session.renew();
            session.insert_user_id(user_id).map_err(|e| {
                let e = LoginError::UnexpectedError(e.into());
                build_err_resp(&secret.0, e)
            })?;
            Ok(HttpResponse::Ok().finish())
        }
        Err(e) => {
            let e = match e {
                AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
                AuthError::UnexpectedError(_) => LoginError::UnexpectedError(e.into()),
            };
            Err(build_err_resp(&secret.0, e))
        }
    }
}
