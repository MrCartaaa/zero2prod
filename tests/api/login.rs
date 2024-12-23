use crate::helpers::spawn_app;

#[tokio::test]
async fn unauthorized_on_bad_credentials() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": "bad_name",
        "password": "bad_password",
    });
    let resp = app.post_login(&login_body).await;

    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn authorized_on_good_credentials() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    });
    let resp = app.post_login(&login_body).await;

    assert_eq!(resp.status().as_u16(), 200);
}