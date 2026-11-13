use axum::Router;
use openhome_api::services::optcg::Optcg;
use serde_json::json;

use super::serve;

pub async fn optcg() -> Optcg {
    let variant = |host: &str, image_id: &str, name: &str| {
        json!({
            "inventory_price": 1.95, "market_price": 2.25, "card_name": name, "set_name": "Synthetic Set",
            "card_text": "[Your Turn] Gain +1000 power.", "set_id": "OP-99", "rarity": "L",
            "card_set_id": "OP99-001", "card_color": "Red", "card_type": "Leader", "life": "5",
            "card_cost": null, "card_power": "5000", "sub_types": "Invented Crew", "counter_amount": null,
            "attribute": "Slash", "date_scraped": "2026-11-12", "card_image_id": image_id,
            "card_image": format!("http://{host}/media/static/Card_Images/{image_id}.jpg"),
        })
    };
    let provider = Router::new()
        .route(
            "/api/sets/card/{code}/",
            axum::routing::get(
                move |axum::extract::Path(code): axum::extract::Path<String>,
                      headers: axum::http::HeaderMap| {
                    let found = code == "OP99-001";
                    let host = headers[axum::http::header::HOST]
                        .to_str()
                        .unwrap()
                        .to_owned();
                    let cards = json!([
                        variant(&host, "OP99-001", "Invented Captain (001)"),
                        variant(&host, "OP99-001_p1", "Invented Captain (001) (Parallel)"),
                    ]);
                    async move {
                        if found {
                            (axum::http::StatusCode::OK, axum::Json(cards))
                        } else {
                            (
                                axum::http::StatusCode::NOT_FOUND,
                                axum::Json(json!({"error": "Card not found."})),
                            )
                        }
                    }
                },
            ),
        )
        .route(
            "/media/static/Card_Images/{file}",
            axum::routing::get(|| async { include_bytes!("../fixtures/pixel.png").as_slice() }),
        );
    let url = format!("http://{}/", serve(provider).await);
    Optcg::new(url.parse().unwrap()).unwrap()
}
