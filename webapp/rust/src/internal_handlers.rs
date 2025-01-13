use axum::extract::State;
use axum::http::StatusCode;

use crate::models::{Chair, Ride};
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
    // MEMO: 一旦最も待たせているリクエストに適当な空いている椅子マッチさせる実装とする。おそらくもっといい方法があるはず…
    let Some(ride): Option<Ride> =
        sqlx::query_as("SELECT * FROM rides WHERE chair_id IS NULL ORDER BY created_at LIMIT 1")
            .fetch_optional(&pool)
            .await?
    else {
        return Ok(StatusCode::NO_CONTENT);
    };

    for _ in 0..10 {
        // おおよそ近いものを持ってくる
        let Some(matched): Option<Chair> = ({
            let m: Option<Chair> = sqlx::query_as(
                r#"
                SELECT
                    c.*
                FROM chairs c
                INNER JOIN chair_total_distance ctd ON c.id=ctd.chair_id
                WHERE
                    ctd.hash = ?
                    AND c.is_active = TRUE
                ORDER BY RAND()
                LIMIT 1"#,
            )
            .bind(knn::coord_to_hash(
                ride.pickup_latitude,
                ride.pickup_longitude,
            ))
            .fetch_optional(&pool)
            .await?;

            match m {
                Some(_) => m,
                None => {
                    tracing::info!(
                        "No suitable chair Found for ride ({:#?}, {:#?}). Using random instead.",
                        ride.pickup_latitude,
                        ride.pickup_longitude
                    );
                    sqlx::query_as(
                        r#"
                        SELECT
                            c.*
                        FROM chairs c
                        INNER JOIN chair_total_distance ctd ON c.id=ctd.chair_id
                        WHERE
                            ctd.zone = ?
                            AND c.is_active = TRUE
                        ORDER BY RAND()
                        LIMIT 1;
                        "#,
                    )
                    .bind(knn::coord_to_zone(
                        ride.pickup_latitude,
                        ride.pickup_longitude,
                    ))
                    .fetch_optional(&pool)
                    .await?
                }
            }
        }) else {
            return Ok(StatusCode::NO_CONTENT);
        };

        let empty: bool = sqlx::query_scalar(
            "SELECT COUNT(*) = 0 FROM (SELECT COUNT(chair_sent_at) = 6 AS completed FROM ride_statuses WHERE ride_id IN (SELECT id FROM rides WHERE chair_id = ?) GROUP BY ride_id) is_completed WHERE completed = FALSE",
        )
        .bind(&matched.id)
        .fetch_one(&pool)
        .await?;

        if empty {
            sqlx::query("UPDATE rides SET chair_id = ? WHERE id = ?")
                .bind(matched.id)
                .bind(ride.id)
                .execute(&pool)
                .await?;
            break;
        }
    }

    Ok(StatusCode::NO_CONTENT)
}
