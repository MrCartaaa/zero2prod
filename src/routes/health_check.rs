use crate::configuration::DatabaseSettings;
use actix_web::{HttpRequest, HttpResponse, Responder};
use sqlx::{Connection, Executor, PgConnection, PgPool};

pub async fn health_check(_req: HttpRequest) -> impl Responder {
    HttpResponse::Ok().finish()
}
