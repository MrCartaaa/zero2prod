use crate::helpers::spawn_app;

#[tokio::test]
async fn confirmations_without_token_return_400() {
    let app = spawn_app().await;

    let resp = reqwest::get(&format!("{}/subscriptions/confirm", &app.address))
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 400);
}
