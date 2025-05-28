use crate::session_state::TypedSession;
use crate::utils::e500;
use actix_web::HttpResponse;

pub async fn logout(session: TypedSession) -> Result<HttpResponse, actix_web::Error> {
    if session.get_user_id().map_err(e500)?.is_some() {
        session.log_out();
    }
    Ok(HttpResponse::Ok().finish())
}
