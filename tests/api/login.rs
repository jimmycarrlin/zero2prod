use crate::helpers::spawn_app;
use crate::helpers::assert_is_redirect_to;

#[tokio::test]
async fn an_error_flash_message_is_set_on_failure() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": "random-username",
        "password": "random-password"
    });
    let response = app.post_login(&login_body).await;
    let flash_cookie = response.cookies().find(|c| c.name() == "_flash").unwrap();

    assert_is_redirect_to(&response, "/login");
    assert_eq!(flash_cookie.value(), "Authentication failed");


    let html_page = app.get_login_html().await;
    assert!(html_page.contains(r#"<p><i>Authentication failed</i></p>"#));
}