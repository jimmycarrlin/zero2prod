use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[tracing::instrument(
name = "Confirm a pending subscriber",
skip(parameters, pool),
)]
pub async fn confirm(parameters: web::Query<Parameters>, pool: web::Data<PgPool>) -> HttpResponse {
    let subscriber_id = match get_subscriber_id_from_token(
        &parameters.subscription_token,
        &pool
    )
    .await {
        Ok(Some(id)) => id,
        Ok(_) => return HttpResponse::Unauthorized().finish(),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    if let Err(_) = confirm_subscriber(subscriber_id, &pool).await {
        return HttpResponse::InternalServerError().finish()
    }

    HttpResponse::Ok().finish()
}

#[tracing::instrument(
name = "Get subscriber_id from token",
skip(subscription_token, pool)
)]
pub async fn get_subscriber_id_from_token(
    subscription_token: &str,
    pool: &PgPool
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT subscriber_id FROM subscription_tokens
            WHERE subscription_token = $1
        "#,
        subscription_token
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

    Ok(result.map(|r| r.subscriber_id))
}

#[tracing::instrument(
name = "Mark subscriber as confirmed",
skip(subscriber_id, pool)
)]
pub async fn confirm_subscriber(subscriber_id: Uuid, pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE subscriptions SET status = 'confirmed'
            WHERE id = $1
        "#,
        subscriber_id
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

    Ok(())
}
