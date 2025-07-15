use actix_session::{Session, SessionExt, SessionGetError, SessionInsertError};
use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest};
use std::future::{ready, Ready};
use std::str::FromStr;
use uuid::Uuid;

pub struct TypedSession(Session);

impl TypedSession {
    const USER_ID_KEY: &'static str = "user_id";

    pub fn renew(&self) {
        self.0.renew();
    }

    pub fn log_out(self) {
        self.0.purge();
    }

    pub fn insert_user_id(&self, _user_id: Uuid) -> Result<(), SessionInsertError> {
        // WARNING: overriding session value for testing -- this might have to be perminently
        // implemented for more involved testing... at which time an auth_service might require
        // implementation
        self.0.insert(Self::USER_ID_KEY, 1)
    }

    pub fn get_user_id(&self) -> Result<Option<Uuid>, SessionGetError> {
        match self.0.get::<String>(Self::USER_ID_KEY) {
            Ok(Some(user_id)) => Ok(Some(Uuid::from_str(user_id.clone().as_str()).unwrap())),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl FromRequest for TypedSession {
    type Error = <Session as FromRequest>::Error;

    type Future = Ready<Result<TypedSession, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        ready(Ok(TypedSession(req.get_session())))
    }
}
