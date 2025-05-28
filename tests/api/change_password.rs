use crate::helpers::spawn_app;
use uuid::Uuid;

#[tokio::test]
async fn you_must_be_logged_in_to_change_your_password() {
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();

    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": Uuid::new_v4().to_string(),
            "new_password": &new_password,
            "new_password_check": &new_password,
        }))
        .await;

    assert_eq!(response.status().as_u16(), 401);
}

#[tokio::test]
async fn new_password_fields_must_match() {
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();
    let another_new_password = Uuid::new_v4().to_string();

    let login_resp = app
        .post_login(&serde_json::json!({
            "username": &app.test_user.username,
            "password": &app.test_user.password,
        }))
        .await;

    assert_eq!(login_resp.status().as_u16(), 200);

    let resp = app
        .post_change_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": &new_password,
            "new_password_check": &another_new_password,
        }))
        .await;

    assert_eq!(*&resp.status().as_u16(), 400);
    assert_eq!(
        resp.text().await.unwrap(),
        "new password must be confirmed."
    );
}

#[tokio::test]
async fn new_password_should_meet_owasp_minimum_requirements() {
    let app = spawn_app().await;
    let new_password = "short-pass".to_string();

    let login_resp = app
        .post_login(&serde_json::json!({
            "username": &app.test_user.username,
            "password": &app.test_user.password,
        }))
        .await;

    assert_eq!(login_resp.status().as_u16(), 200);

    let resp = app
        .post_change_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": &new_password,
            "new_password_check": &new_password,
        }))
        .await;
    assert_eq!(*&resp.status().as_u16(), 400);
    assert_eq!(
        resp.text().await.unwrap(),
        "new password must meet requirements."
    );
}

#[tokio::test]
async fn changing_password_works() {
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();

    let login_resp = app
        .post_login(&serde_json::json!({
            "username": &app.test_user.username,
            "password": &app.test_user.password,
        }))
        .await;

    assert_eq!(login_resp.status().as_u16(), 200);

    let resp = app
        .post_change_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": &new_password,
            "new_password_check": &new_password,
        }))
        .await;

    assert_eq!(*&resp.status().as_u16(), 200);

    let resp = app.post_logout().await;
    assert_eq!(resp.status().as_u16(), 200);

    let login_resp = app
        .post_login(&serde_json::json!({
            "username": &app.test_user.username,
            "password": &new_password,
        }))
        .await;

    assert_eq!(login_resp.status().as_u16(), 200);
}
