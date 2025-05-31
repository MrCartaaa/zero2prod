use crate::authentication::reject_anonymous_users;
use crate::cloneable_auth_token::SecretAuthToken;
use crate::configuration::{DatabaseSettings, Settings};
use crate::email_client::EmailClient;
use crate::routes::{
    change_password, confirm, health_check, login, logout, publish_newsletter, subscribe,
};
use actix_session::storage::RedisSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::Key;
use actix_web::dev::Server;
use actix_web::middleware::from_fn;
use actix_web::{web, App, HttpServer};
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(config: Settings) -> Result<Self, anyhow::Error> {
        let connection_pool = get_connection_pool(&config.database);
        let email_client = config.email_client.client();

        let address = format!("{}:{}", config.application.host, config.application.port);

        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(
            listener,
            connection_pool,
            email_client,
            config.application.base_url,
            config.application.hmac_secret,
            config.redis_uri,
        )
        .await?;

        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub fn get_connection_pool(config: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new().connect_lazy_with(config.connect_options())
}

pub struct ApplicationBaseUrl(pub String);

#[derive(Clone)]
pub struct HmacSecret(pub SecretAuthToken);

async fn run(
    listener: TcpListener,
    db_pool: PgPool,
    email_client: EmailClient,
    base_url: String,
    hmac_secret: SecretAuthToken,
    redis_uri: SecretAuthToken,
) -> Result<Server, anyhow::Error> {
    let db_pool = web::Data::new(db_pool);
    let email_client = web::Data::new(email_client);
    let base_url = web::Data::new(ApplicationBaseUrl(base_url));
    let secret_key = Key::from(hmac_secret.expose_secret().token.as_bytes());
    let redis_store = RedisSessionStore::new(redis_uri.expose_secret().clone().token).await?;
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .wrap(SessionMiddleware::new(
                redis_store.clone(),
                secret_key.clone(),
            ))
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .route(
                "/newsletters",
                web::post()
                    .to(publish_newsletter)
                    .wrap(from_fn(reject_anonymous_users)),
            )
            .route("/login", web::post().to(login))
            .route(
                "/password",
                web::post()
                    .to(change_password)
                    .wrap(from_fn(reject_anonymous_users)),
            )
            .route(
                "/logout",
                web::post().to(logout).wrap(from_fn(reject_anonymous_users)),
            )
            .app_data(db_pool.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
            .app_data(web::Data::new(HmacSecret(hmac_secret.clone())))
    })
    .listen(listener)?
    .run();
    Ok(server)
}
