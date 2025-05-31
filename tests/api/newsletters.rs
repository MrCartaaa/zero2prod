use crate::helpers::{spawn_app, ConfirmationLinks, TestApp};
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::Name;
use fake::Fake;
use std::time::Duration;
use wiremock::matchers::{any, method, path};
use wiremock::MockBuilder;
use wiremock::{Mock, ResponseTemplate};

fn when_sending_an_email() -> MockBuilder {
    Mock::given(path("/email")).and(method("POST"))
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    });
    let resp = app.post_login(&login_body).await;

    assert_eq!(resp.status().as_u16(), 200);

    create_confirmed_subscriber(&app).await;

    when_sending_an_email()
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "newsletter",
        "content": {
            "text": "Newsletter body",
            "html": "<p>Newsletter body</p>",
        }, "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    let resp = app.post_newsletters(&newsletter_request_body).await;
    assert_eq!(resp.status().as_u16(), 200);
    app.dispatch_all_pending_emails().await;
}

#[tokio::test]
async fn newsletters_returns_400_for_invalid_data() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    });
    let resp = app.post_login(&login_body).await;

    assert_eq!(resp.status().as_u16(), 200);

    let test_cases = vec![
        (
            serde_json::json!({
                "title": "newsletter",
            }),
            "missing content",
        ),
        (
            serde_json::json!({
                "content":  {
                    "text": "Newsletter body",
                    "html": "<p>Newsletter body</p>",
                },
            }),
            "missing title",
        ),
    ];

    for (invalid_body, error_msg) in test_cases {
        let resp = app.post_newsletters(&invalid_body).await;

        assert_eq!(
            resp.status().as_u16(),
            400,
            "The API did not fail with 400 Bad request when the payload was {}.",
            error_msg
        );
    }
}

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    });
    let resp = app.post_login(&login_body).await;

    assert_eq!(resp.status().as_u16(), 200);

    create_unconfirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "newsletter",
        "content": {
            "text": "Newsletter body",
            "html": "<p>Newsletter body</p>",
        }, "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    let resp = app.post_newsletters(&newsletter_request_body).await;
    assert_eq!(resp.status().as_u16(), 200);
    app.dispatch_all_pending_emails().await;
}

#[tokio::test]
async fn newsletter_is_idempotent() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    app.test_user.login(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newletter Title",
        "content": {
        "text": "body",
        "html": "<p> body </p>"
    }, "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let resp = app.post_newsletters(&newsletter_request_body).await;
    assert_eq!(resp.status().as_u16(), 200);

    let resp = app.post_newsletters(&newsletter_request_body).await;
    assert_eq!(*&resp.status().as_u16(), 400);
    assert_eq!(
        resp.text().await.unwrap(),
        "The newsletter has already been posted."
    );
    app.dispatch_all_pending_emails().await;
}

// Removed as emails are being sent via queue
// #[tokio::test]
// async fn transient_errors_do_not_cause_duplicate_deliveries_on_retries() {
//     let app = spawn_app().await;
//
//     let newsletter_request_body = serde_json::json!({
//         "title": "title",
//         "content": {
//             "text": "body",
//             "html": "<p>body</p>"
//         },
//         "idempotency_key": uuid::Uuid::new_v4().to_string()
//     });
//
//     create_confirmed_subscriber(&app).await;
//     create_confirmed_subscriber(&app).await;
//     app.test_user.login(&app).await;
//
//     when_sending_an_email()
//         .respond_with(ResponseTemplate::new(200))
//         .up_to_n_times(1)
//         .expect(1)
//         .mount(&app.email_server)
//         .await;
//     when_sending_an_email()
//         .respond_with(ResponseTemplate::new(500))
//         .up_to_n_times(1)
//         .expect(1)
//         .mount(&app.email_server)
//         .await;
//
//     let resp = app.post_newsletters(&newsletter_request_body).await;
//     assert_eq!(resp.status().as_u16(), 200);
//
//     when_sending_an_email()
//         .respond_with(ResponseTemplate::new(200))
//         .expect(1)
//         .named("Delivery Retry")
//         .mount(&app.email_server)
//         .await;
//
//     let resp = app.post_newsletters(&newsletter_request_body).await;
//     assert_eq!(*&resp.status().as_u16(), 200);
// }

#[tokio::test]
async fn concurrent_form_submission_is_handled_gracefully() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    app.test_user.login(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "title",
        "content": {
            "text": "body",
            "html": "<p>body</p>"
        },
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let resp1 = app.post_newsletters(&newsletter_request_body);
    let resp2 = app.post_newsletters(&newsletter_request_body);
    let (resp1, resp2) = tokio::join!(resp1, resp2);

    let s1 = *&resp1.status().as_u16();
    let s2 = *&resp2.status().as_u16();

    if s1 == 200 {
        assert_eq!(s2, 400);
        assert_eq!(
            resp2.text().await.unwrap(),
            "The newsletter has already been posted."
        );
    } else {
        if s2 == 200 {
            assert_eq!(s1, 400);
            assert_eq!(
                resp1.text().await.unwrap(),
                "The newsletter has already been posted."
            );
        } else {
            assert!(
                false,
                "None of the responses returned with status code 200."
            );
        }
    }
    app.dispatch_all_pending_emails().await;
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let name: String = Name().fake();
    let email: String = SafeEmail().fake();
    let body = serde_urlencoded::to_string(&serde_json::json!(
    {    "name": name,
        "email":email
    }))
    .unwrap();

    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    app.post_subscription(body.into())
        .await
        .error_for_status()
        .unwrap();

    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();

    app.get_confirmation_links(&email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_links = create_unconfirmed_subscriber(app).await;
    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}

#[tokio::test]
async fn requests_missing_authorization_are_rejected() {
    let app = spawn_app().await;

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
