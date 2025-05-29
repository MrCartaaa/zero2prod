use crate::authentication::UserId;
use crate::session_state::TypedSession;
use actix_web::{web, HttpResponse};

pub async fn logout(
    _: web::ReqData<UserId>,
    session: TypedSession,
) -> Result<HttpResponse, actix_web::Error> {
    session.log_out();
    Ok(HttpResponse::Ok().finish())
}
