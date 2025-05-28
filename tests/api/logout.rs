use crate::helpers::spawn_app;

#[tokio::test]
async fn logout_clears_session_state() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    });

    let resp = app.post_login(&login_body).await;

    assert_eq!(resp.status().as_u16(), 200);

    let resp = app.post_logout().await;
    assert_eq!(resp.status().as_u16(), 200);

    let resp = reqwest::Client::new()
        .post(&format!("{}/newsletters", app.address))
        .json(&serde_json::json! {{
            "title": "newsletter",
            "content": {
                "text": "Newsletter body",
                "html": "<p>Newsletter body</p>",
            }
        }})
        .send()
        .await
        .expect("Request failed");

    assert_eq!(401, resp.status().as_u16());
}
