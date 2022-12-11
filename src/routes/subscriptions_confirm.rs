use std::fmt::{Debug, Display, Formatter};
use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;
use anyhow::Context;

use crate::routes::subscriptions::error_chain_fmt;

#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[derive(thiserror::Error)]
pub enum ConfirmError {
    #[error("There is no subscriber associated with the provided token")]
    UnknownToken,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl Debug for ConfirmError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(f, self)
    }
}


#[tracing::instrument(
name = "Confirm a pending subscriber",
skip(parameters, pool),

)]
pub async fn confirm(
    parameters: web::Query<Parameters>,
    pool: web::Data<PgPool>
) -> Result<HttpResponse, ConfirmError> {
    let subscriber_id = get_subscriber_id_from_token(&parameters.subscription_token, &pool)
        .await
        .context("Failed to get subscriber id from authorization token")?
        .ok_or(ConfirmError::UnknownToken)?;

    confirm_subscriber(subscriber_id, &pool)
        .await
        .context("Failed to update the subscriber status to `confirmed`")?;

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(
name = "Get subscriber_id from token",
skip(subscription_token, pool)
)]
pub async fn get_subscriber_id_from_token(
    subscription_token: &str,
    pool: &PgPool
) -> Result<Option<Uuid>, sqlx::Error> {
    let entry = sqlx::query!(
        r#"
        SELECT subscriber_id FROM subscription_tokens
            WHERE subscription_token = $1
        "#,
        subscription_token
    )
    .fetch_optional(pool)
    .await?;

    Ok(entry.map(|e| e.subscriber_id))
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
    .await?;

    Ok(())
}
