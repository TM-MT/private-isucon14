use axum::extract::State;
use axum::http::StatusCode;

use crate::models::Ride;
use crate::{knn, AppState, Error};

pub fn internal_routes() -> axum::Router<AppState> {
    axum::Router::new().route(
        "/api/internal/matching",
        axum::routing::get(internal_get_matching),
    )
}

// このAPIをインスタンス内から一定間隔で叩かせることで、椅子とライドをマッチングさせる
async fn internal_get_matching(
    State(AppState { pool, .. }): State<AppState>,
) -> Result<StatusCode, Error> {
    let rides: Vec<Ride> =
        sqlx::query_as("SELECT * FROM rides WHERE chair_id IS NULL ORDER BY created_at LIMIT 10")
            .fetch_all(&pool)
            .await?;

    for ride in rides {
        let mut match_candidates: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT
                c.id
            FROM chairs c
            INNER JOIN chair_total_distance ctd ON c.id=ctd.chair_id
            WHERE
                ctd.hash = ?
                AND c.is_active = TRUE
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
                    WHERE completed = FALSE)
            ORDER BY (ABS(ctd.last_latitude - ?) + ABS(ctd.last_longitude - ?)) ASC
            LIMIT 1"#,
        )
        .bind(knn::coord_to_hash(
            ride.pickup_latitude,
            ride.pickup_longitude,
        ))
        .bind(ride.pickup_latitude)
        .bind(ride.pickup_longitude)
        .fetch_all(&pool)
        .await?;

        if match_candidates.is_empty() {
            tracing::info!(
                "No suitable chair Found for ride ({:#?}, {:#?}). Using random instead.",
                ride.pickup_latitude,
                ride.pickup_longitude
            );
            match_candidates = sqlx::query_as(
                r#"
                SELECT
                    c.*
                FROM chairs c
                INNER JOIN chair_total_distance ctd ON c.id=ctd.chair_id
                WHERE
                    ctd.zone = ?
                    AND c.is_active = TRUE
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
                        WHERE completed = FALSE)
                ORDER BY RAND()
                LIMIT 1;
                "#,
            )
            .bind(knn::coord_to_zone(
                ride.pickup_latitude,
                ride.pickup_longitude,
            ))
            .fetch_all(&pool)
            .await?;
        }

        if let Some((matched,)) = match_candidates.into_iter().next() {
            sqlx::query("UPDATE rides SET chair_id = ? WHERE id = ?")
                .bind(matched)
                .bind(ride.id)
                .execute(&pool)
                .await?;
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
