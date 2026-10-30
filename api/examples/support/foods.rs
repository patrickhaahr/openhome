//! A synthetic Food database for the dashboard fixtures: a handful of invented Foods with
//! plausible values, recorded as a complete import of the pinned dataset so lookups answer
//! without downloading anything. Not real DTU values.

use openhome_api::services::nutrition::{
    food_database::{DatasetPin, FoodDatabase},
    meal_photos::MealPhotos,
};
use sqlx::sqlite::SqlitePoolOptions;

/// The Food database's state in a fixture.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FoodState {
    /// A complete import of the synthetic Foods.
    Ready,
    /// An empty `food.db` whose import never runs, so lookups report importing.
    Importing,
}

impl FoodState {
    /// `--food-database=importing` keeps the fixture's Food database importing.
    pub fn from_args() -> Self {
        if std::env::args().any(|argument| argument == "--food-database=importing") {
            Self::Importing
        } else {
            Self::Ready
        }
    }
}

const PARAMETERS: &str = "INSERT INTO parameter (parameter_id, key, name_da, name_en, group_da, \
    group_en, unit, per_100g, sort_key) VALUES \
    (137, 'energy_kj', 'Energi (kJ)', 'Energy (kJ)', 'Makronæringstoffer m.m.', 'Macro nutrients etc.', 'kJ', 1, 1010), \
    (356, 'energy_kcal', 'Energi (kcal)', 'Energy (kcal)', 'Makronæringstoffer m.m.', 'Macro nutrients etc.', 'kcal', 1, 1030), \
    (218, 'protein', 'Protein', 'Protein', 'Makronæringstoffer m.m.', 'Macro nutrients etc.', 'g', 1, 1110), \
    (172, 'available_carbohydrate', 'Tilgængelig kulhydrat', 'Available carbohydrates', 'Makronæringstoffer m.m.', 'Macro nutrients etc.', 'g', 1, 1220), \
    (168, 'dietary_fibre', 'Kostfibre', 'Dietary fibre', 'Makronæringstoffer m.m.', 'Macro nutrients etc.', 'g', 1, 1240), \
    (141, 'fat', 'Fedt', 'Fat', 'Makronæringstoffer m.m.', 'Macro nutrients etc.', 'g', 1, 1310), \
    (19, 'alcohol', 'Alkohol', 'Alcohol', 'Makronæringstoffer m.m.', 'Macro nutrients etc.', 'g', 1, 1410), \
    (126, 'vitamin_d', 'D-vitamin', 'Vitamin D', 'Vitaminer', 'Vitamins', 'µg', 1, 2110), \
    (100, 'vitamin_c', 'C-vitamin', 'Vitamin C', 'Vitaminer', 'Vitamins', 'mg', 1, 2310), \
    (55, 'calcium', 'Calcium', 'Calcium', 'Mineraler og uorganiske stoffer', 'Minerals and inorganic substances', 'mg', 1, 3110), \
    (58, 'iron', 'Jern', 'Iron', 'Mineraler og uorganiske stoffer', 'Minerals and inorganic substances', 'mg', 1, 3210), \
    (252, 'waste', 'Svind', 'Waste', 'Faktorer m.m.', 'Factors etc.', '%', 0, 20010), \
    (363, 'density', 'Massefylde', 'Density', 'Faktorer m.m.', 'Factors etc.', 'g/ml', 0, 20210)";

const FOODS: &str = "INSERT INTO food (food_id, name_da, name_en, food_group_id, food_group_da, \
    food_group_en) VALUES \
    (1693, 'Skyr, 0.2 % fedt', 'Skyr, 0.2 % fat', 3, 'Syrnede mælkeprodukter', 'Fermented milk products'), \
    (60, 'Havregryn', 'Oat flakes', 20, 'Morgenmadsprodukter', 'Breakfast cereals'), \
    (3, 'Banan, rå', 'Banana, raw', 52, 'Sydfrugt', 'Tropical or subtropical fruit'), \
    (907, 'Kylling, bryst, kød, rå', 'Chicken, breast, flesh, raw', 80, 'Fjerkræ', 'Poultry'), \
    (300, 'Ris, hvide, kogte', 'Rice, white, boiled', 21, 'Ris', 'Rice')";

// (food_id, parameter_id, value per 100 g); a missing pair is unknown.
const VALUES: &str = "INSERT INTO food_value (food_id, parameter_id, value) VALUES \
    (1693, 137, 260), (1693, 356, 62), (1693, 218, 11), (1693, 172, 4), (1693, 168, 0), \
    (1693, 141, 0.2), (1693, 19, 0), (1693, 126, 0), (1693, 55, 120), (1693, 363, 1.03), \
    (60, 137, 1550), (60, 356, 370), (60, 218, 13), (60, 172, 58), (60, 168, 10), (60, 141, 7), \
    (60, 19, 0), (60, 58, 4), (60, 55, 50), \
    (3, 356, 90), (3, 218, 1.1), (3, 172, 20), (3, 168, 2.6), (3, 141, 0.3), (3, 100, 9), \
    (3, 252, 35), \
    (907, 356, 110), (907, 218, 23), (907, 141, 1.5), (907, 126, 0.1), (907, 58, 0.5), \
    (300, 356, 130), (300, 218, 2.7), (300, 172, 28), (300, 168, 0.4), (300, 141, 0.3)";

/// A Food database in memory, as `state` describes it.
pub async fn synthetic_foods(state: FoodState) -> anyhow::Result<FoodDatabase> {
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    let pin = DatasetPin::pinned();
    if state == FoodState::Ready {
        sqlx::migrate!("./food_migrations").run(&db).await?;
        sqlx::query(
            "INSERT INTO dataset (id, version, sha256, imported_at) \
             VALUES (1, ?, ?, '2026-10-30T00:00:00Z')",
        )
        .bind(&pin.version)
        .bind(&pin.sha256)
        .execute(&db)
        .await?;
        for statement in [PARAMETERS, FOODS, VALUES] {
            sqlx::query(statement).execute(&db).await?;
        }
    }
    let unused_cache = std::env::temp_dir().join("openhome-dashboard-fixture-food-dataset");
    let (foods, _import) = FoodDatabase::open(db, pin, unused_cache).await?;
    Ok(foods)
}

/// Meal photos in a temporary directory that lives as long as the returned guard.
pub fn photo_store() -> anyhow::Result<(MealPhotos, tempfile::TempDir)> {
    let dir = tempfile::Builder::new()
        .prefix("openhome-fixture-meal-photos")
        .tempdir()?;
    Ok((MealPhotos::new(dir.path().to_path_buf()), dir))
}

/// A drawn plate of `(red, green, blue)` food on a wooden table, as a base64 PNG: no real photo.
pub fn synthetic_plate(food: [u8; 3]) -> String {
    use base64::Engine;
    use image::ImageEncoder;

    let (width, height) = (1200_u32, 900_u32);
    let plate = image::RgbImage::from_fn(width, height, |x, y| {
        let (dx, dy) = (f64::from(x) - 600.0, f64::from(y) - 450.0);
        let distance = dx.hypot(dy);
        if distance < 220.0 + 30.0 * (dx.atan2(dy) * 5.0).sin() {
            image::Rgb(food)
        } else if distance < 380.0 {
            image::Rgb([236, 232, 224])
        } else {
            // Wood grain: bands of six shades running across the table.
            let grain = ((x / 90 + y / 14) % 6) as u8 * 4;
            image::Rgb([138 + grain, 92 + grain, 60 + grain / 2])
        }
    });
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(
            plate.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgb8,
        )
        .expect("a synthetic PNG always encodes");
    base64::engine::general_purpose::STANDARD.encode(png)
}

/// A Portion's item: a Food by ID, or the synthetic Open Food Facts product.
#[derive(Clone, Copy)]
enum Item {
    Food(i64),
    Product,
}

/// Hours before now, an optional note, `(item, grams)` Portions and an optional plate colour.
type SyntheticMeal = (
    i64,
    Option<&'static str>,
    &'static [(Item, f64)],
    Option<[u8; 3]>,
);

/// Invented Meal Logs over the last two days, logged through the production service so they
/// carry real snapshots and photos, for previews that should open on a populated day. The
/// synthetic product is linked to Skyr first, so one meal holds a Product Portion.
pub async fn seed_meals(
    db: &sqlx::SqlitePool,
    foods: &FoodDatabase,
    photos: &MealPhotos,
    labels: &openhome_api::services::nutrition::open_food_facts::OpenFoodFacts,
) -> anyhow::Result<()> {
    use Item::{Food, Product};
    use openhome_api::services::nutrition::meal_logs::{self, LogMeal, PortionRequest};
    use openhome_api::services::nutrition::products::{self, LinkProduct};

    products::link(
        db,
        foods,
        labels,
        LinkProduct {
            barcode: crate::providers::SYNTHETIC_BARCODE.into(),
            food_id: 1693,
            request_id: "synthetic-product-link".into(),
        },
    )
    .await
    .map_err(|error| anyhow::anyhow!("linking the synthetic product failed: {error}"))?;
    let now = chrono::Utc::now();
    let meals: [SyntheticMeal; 5] = [
        (27, None, &[(Food(1693), 200.0), (Food(60), 50.0)], None),
        (
            22,
            Some("Synthetic note: dinner out"),
            &[(Food(907), 180.0), (Food(300), 250.0)],
            Some([214, 168, 74]),
        ),
        (
            5,
            Some("after the run"),
            &[(Product, 150.0), (Food(60), 60.0), (Food(3), 120.0)],
            Some([238, 226, 196]),
        ),
        (3, None, &[(Food(907), 150.0), (Food(300), 200.0)], None),
        (1, None, &[(Food(3), 110.0)], None),
    ];
    for (index, (hours_ago, note, portions, plate)) in meals.into_iter().enumerate() {
        meal_logs::log_meal(
            db,
            foods,
            photos,
            LogMeal {
                eaten_at: (now - chrono::TimeDelta::hours(hours_ago)).to_rfc3339(),
                note: note.map(str::to_owned),
                portions: portions
                    .iter()
                    .map(|&(item, grams)| PortionRequest {
                        food_id: match item {
                            Food(food_id) => Some(food_id),
                            Product => None,
                        },
                        barcode: matches!(item, Product)
                            .then(|| crate::providers::SYNTHETIC_BARCODE.to_owned()),
                        grams,
                        estimates: std::collections::BTreeMap::new(),
                    })
                    .collect(),
                request_id: format!("synthetic-meal-{index}"),
                photo: plate.map(synthetic_plate),
            },
        )
        .await
        .map_err(|error| anyhow::anyhow!("seeding a synthetic meal failed: {error}"))?;
    }
    Ok(())
}
