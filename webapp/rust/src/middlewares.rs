use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::CookieJar;
use moka::future::Cache;
use sqlx::MySql;
use sqlx::Pool;

use crate::models::{Chair, Owner, User};
use crate::{AppState, Error};

pub async fn app_auth_middleware(
    State(AppState { pool, .. }): State<AppState>,
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Result<Response, Error> {
    let Some(c) = jar.get("app_session") else {
        return Err(Error::Unauthorized("app_session cookie is required"));
    };
    let access_token = c.value();
    let Some(user): Option<User> = sqlx::query_as("SELECT * FROM users WHERE access_token = ?")
        .bind(access_token)
        .fetch_optional(&pool)
        .await?
    else {
        return Err(Error::Unauthorized("invalid access token"));
    };

    req.extensions_mut().insert(user);

    Ok(next.run(req).await)
}

pub async fn owner_auth_middleware(
    State(AppState { pool, .. }): State<AppState>,
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Result<Response, Error> {
    let Some(c) = jar.get("owner_session") else {
        return Err(Error::Unauthorized("owner_session cookie is required"));
    };
    let access_token = c.value();
    let Some(owner): Option<Owner> = sqlx::query_as("SELECT * FROM owners WHERE access_token = ?")
        .bind(access_token)
        .fetch_optional(&pool)
        .await?
    else {
        return Err(Error::Unauthorized("invalid access token"));
    };

    req.extensions_mut().insert(owner);

    Ok(next.run(req).await)
}

pub async fn chair_auth_middleware(
    State(AppState {
        pool, chair_cache, ..
    }): State<AppState>,
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Result<Response, Error> {
    let Some(c) = jar.get("chair_session") else {
        return Err(Error::Unauthorized("chair_session cookie is required"));
    };
    let access_token = c.value();
    let Some(chair): Option<Chair> = get_chair(&pool, &chair_cache, access_token).await else {
        return Err(Error::Unauthorized("invalid access token"));
    };

    req.extensions_mut().insert(chair);

    Ok(next.run(req).await)
}

async fn get_chair(
    pool: &Pool<MySql>,
    cache: &Cache<String, Chair>,
    access_token: &str,
) -> Option<Chair> {
    cache
        .optionally_get_with(access_token.to_string(), async {
            let t: Option<Chair> = sqlx::query_as("SELECT * FROM chairs WHERE access_token = ?")
                .bind(access_token)
                .fetch_optional(pool)
                .await
                .inspect_err(|e| eprintln!("invalid access token: {e}"))
                .unwrap_or(None);
            t
        })
        .await
}
