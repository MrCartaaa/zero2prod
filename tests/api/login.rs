use crate::helpers::spawn_app;
use ring::hmac;
use secrecy::ExposeSecret;

#[tokio::test]
async fn unauthorized_on_bad_credentials() {
    let mut app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": "bad_name",
        "password": "bad_password",
    });
    let resp = app.post_login(&login_body).await;

    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn authorized_on_good_credentials() {
    let mut app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    });
    let resp = app.post_login(&login_body).await;

    assert_eq!(resp.status().as_u16(), 200);
}

#[tokio::test]
async fn digest_expected_resp_body_hmac_on_bad_credentials() {
    let mut app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": "bad_name",
        "password": "bad_password",
    });
    let resp = app.post_login(&login_body).await;

    let resp_vbytes: Vec<u8> =
        serde_json::from_str(resp.text().await.expect("RESULT").as_str()).unwrap();
    let resp_bytes: &[u8] = &resp_vbytes.clone();

    let expected_message = format!("{}", urlencoding::encode("Authentication Failed."));
    let key_val: &[u8] = app.hmac_secret.expose_secret().token.as_bytes();
    let key = hmac::Key::new(hmac::HMAC_SHA256, key_val.as_ref());

    assert!(hmac::verify(&key, &expected_message.as_bytes(), resp_bytes).is_ok());
}
