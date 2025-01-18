use std::collections::HashSet;

use crate::models::{ChairLocation, Ride};
use crate::{AppState, Error};
use axum::extract::State;
use axum::http::StatusCode;

pub fn internal_routes() -> axum::Router<AppState> {
    axum::Router::new().route(
        "/api/internal/matching",
        axum::routing::get(internal_get_matching),
    )
}

const MATCH_TRESHOLD: i32 = 200;

// このAPIをインスタンス内から一定間隔で叩かせることで、椅子とライドをマッチングさせる
async fn internal_get_matching(
    State(AppState {
        pool,
        internal_matching_lock,
        ..
    }): State<AppState>,
) -> Result<StatusCode, Error> {
    let semaphore = internal_matching_lock.clone();

    if semaphore.try_acquire().is_err() {
        return Ok(StatusCode::NO_CONTENT);
    }

    let rides: Vec<Ride> =
        sqlx::query_as("SELECT * FROM rides WHERE chair_id IS NULL ORDER BY created_at")
            .fetch_all(&pool)
            .await?;
    if rides.is_empty() {
        return Ok(StatusCode::NO_CONTENT);
    }

    let available_chairs: Vec<ChairLocation> = sqlx::query_as(
        r#"
        SELECT
            ctd.chair_id,
            ctd.last_latitude AS latitude,
            ctd.last_longitude AS longitude
        FROM chairs c
        INNER JOIN chair_total_distance ctd ON c.id=ctd.chair_id
        WHERE
            c.is_active = TRUE
            AND (SELECT 
                    COUNT(*) = 0 
                FROM (
                    SELECT 
                        COUNT(chair_sent_at) = 6 AS completed 
                    FROM ride_statuses 
                    WHERE ride_id IN (
                        SELECT id FROM rides WHERE chair_id = c.id
                    )
                    GROUP BY ride_id
                ) is_completed 
                WHERE completed = FALSE)"#,
    )
    .fetch_all(&pool)
    .await?;

    let mut matched_chairs = HashSet::new();

    // Greedyにマッチさせる
    for ride in rides {
        let candidate: Option<(&ChairLocation, i32)> = available_chairs
            .iter()
            .filter_map(|chair| {
                if matched_chairs.contains(&chair.chair_id) {
                    None
                } else {
                    Some((
                        chair,
                        crate::calculate_distance(
                            ride.pickup_latitude,
                            ride.pickup_longitude,
                            chair.latitude,
                            chair.longitude,
                        ),
                    ))
                }
            })
            .min_by_key(|e| e.1);
        if let Some((cl, distance)) = candidate {
            if distance < MATCH_TRESHOLD {
                let res = sqlx::query("UPDATE rides SET chair_id = ? WHERE id = ?")
                    .bind(cl.chair_id.clone())
                    .bind(ride.id)
                    .execute(&pool)
                    .await;
                matched_chairs.insert(cl.chair_id.clone());
                if res.is_err() {
                    tracing::info!("Failed to update rides: {:?}", res);
                }
            } else {
                tracing::info!("Too far to pickup: distance={:?}", distance);
            }
        } else {
            tracing::info!(
                "No match found for ride ({:?}, {:?})",
                ride.pickup_latitude,
                ride.pickup_longitude
            );
        }
    }

    Ok(StatusCode::NO_CONTENT)
}
