use openhome_api::services::nutrition::{
    food_database::{DatasetPin, FoodDatabase},
    meal_photos::MealPhotos,
};
use sqlx::sqlite::SqlitePoolOptions;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FoodState {
    Ready,
    Importing,
}

impl FoodState {
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

const VALUES: &str = "INSERT INTO food_value (food_id, parameter_id, value) VALUES \
    (1693, 137, 260), (1693, 356, 62), (1693, 218, 11), (1693, 172, 4), (1693, 168, 0), \
    (1693, 141, 0.2), (1693, 19, 0), (1693, 126, 0), (1693, 55, 120), (1693, 363, 1.03), \
    (60, 137, 1550), (60, 356, 370), (60, 218, 13), (60, 172, 58), (60, 168, 10), (60, 141, 7), \
    (60, 19, 0), (60, 58, 4), (60, 55, 50), \
    (3, 356, 90), (3, 218, 1.1), (3, 172, 20), (3, 168, 2.6), (3, 141, 0.3), (3, 100, 9), \
    (3, 252, 35), \
    (907, 356, 110), (907, 218, 23), (907, 141, 1.5), (907, 126, 0.1), (907, 58, 0.5), \
    (300, 356, 130), (300, 218, 2.7), (300, 172, 28), (300, 168, 0.4), (300, 141, 0.3)";

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

pub fn photo_store() -> anyhow::Result<(MealPhotos, tempfile::TempDir)> {
    let dir = tempfile::Builder::new()
        .prefix("openhome-fixture-meal-photos")
        .tempdir()?;
    Ok((MealPhotos::new(dir.path().to_path_buf()), dir))
}

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

#[derive(Clone, Copy)]
enum Item {
    Food(i64),
    Product,
}

#[derive(Clone, Copy)]
struct Portion {
    item: Item,
    grams: f64,
}

struct HistoryPortion {
    food_id: i64,
    grams: f64,
}

struct HistoryMeal {
    hour: i64,
    portions: Vec<HistoryPortion>,
}

struct SyntheticMeal {
    hours_ago: i64,
    note: Option<&'static str>,
    portions: &'static [Portion],
    plate: Option<[u8; 3]>,
}

pub async fn seed_meals(
    db: &sqlx::SqlitePool,
    foods: &FoodDatabase,
    photos: &MealPhotos,
    labels: &openhome_api::services::nutrition::open_food_facts::OpenFoodFacts,
) -> anyhow::Result<()> {
    use Item::{Food, Product};
    use openhome_api::services::nutrition::meal_logs::{
        self, LogMeal, OverrideRequest, PortionRequest,
    };
    use openhome_api::services::nutrition::meals;
    use openhome_api::services::nutrition::nova;
    use openhome_api::services::nutrition::products::{self, LinkProduct};
    use openhome_api::services::nutrition::supplements::{
        self, AddSupplement, AmountRequest, CompoundRequest, DoseRequest, DoseUnit,
    };

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
    let mut supplement_ids = Vec::new();
    for supplement in [
        AddSupplement {
            name: "Vitamin D3".into(),
            dose_unit: DoseUnit::Capsule,
            amounts: vec![AmountRequest {
                key: "vitamin_d".into(),
                value: 25.0,
                unit: "µg".into(),
                form: None,
            }],
            compounds: vec![],
            request_id: "synthetic-supplement-Vitamin D3".into(),
        },
        AddSupplement {
            name: "Creatine".into(),
            dose_unit: DoseUnit::Scoop,
            amounts: vec![],
            compounds: vec![CompoundRequest {
                name: "Creatine monohydrate".into(),
                value: 3.0,
                unit: "g".into(),
            }],
            request_id: "synthetic-supplement-Creatine".into(),
        },
        AddSupplement {
            name: "Vitamin D 250".into(),
            dose_unit: DoseUnit::Capsule,
            amounts: vec![AmountRequest {
                key: "vitamin_d".into(),
                value: 250.0,
                unit: "µg".into(),
                form: None,
            }],
            compounds: vec![],
            request_id: "synthetic-supplement-Vitamin D 250".into(),
        },
    ] {
        let added = supplements::add_supplement(db, supplement)
            .await
            .map_err(|error| anyhow::anyhow!("adding a synthetic Supplement failed: {error}"))?;
        supplement_ids.push(added.supplement.id);
    }
    let now = chrono::Utc::now();
    let meals: [SyntheticMeal; 5] = [
        SyntheticMeal {
            hours_ago: 27,
            note: None,
            portions: &[
                Portion {
                    item: Food(1693),
                    grams: 200.0,
                },
                Portion {
                    item: Food(60),
                    grams: 50.0,
                },
            ],
            plate: None,
        },
        SyntheticMeal {
            hours_ago: 22,
            note: Some("Synthetic note: dinner out"),
            portions: &[
                Portion {
                    item: Food(907),
                    grams: 180.0,
                },
                Portion {
                    item: Food(300),
                    grams: 250.0,
                },
            ],
            plate: Some([214, 168, 74]),
        },
        SyntheticMeal {
            hours_ago: 5,
            note: Some("after the run"),
            portions: &[
                Portion {
                    item: Product,
                    grams: 150.0,
                },
                Portion {
                    item: Food(60),
                    grams: 60.0,
                },
                Portion {
                    item: Food(3),
                    grams: 120.0,
                },
            ],
            plate: Some([238, 226, 196]),
        },
        SyntheticMeal {
            hours_ago: 3,
            note: None,
            portions: &[
                Portion {
                    item: Food(907),
                    grams: 150.0,
                },
                Portion {
                    item: Food(300),
                    grams: 200.0,
                },
            ],
            plate: None,
        },
        SyntheticMeal {
            hours_ago: 1,
            note: None,
            portions: &[Portion {
                item: Food(3),
                grams: 110.0,
            }],
            plate: None,
        },
    ];
    for (index, meal) in meals.into_iter().enumerate() {
        meal_logs::log_meal(
            db,
            foods,
            photos,
            LogMeal {
                eaten_at: (now - chrono::TimeDelta::hours(meal.hours_ago)).to_rfc3339(),
                note: meal.note.map(str::to_owned),
                portions: Some(
                    meal.portions
                        .iter()
                        .map(|&Portion { item, grams }| PortionRequest {
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
                ),
                supplement_doses: (index == 2).then(|| {
                    supplement_ids[..2]
                        .iter()
                        .map(|id| DoseRequest {
                            supplement_id: id.clone(),
                            doses: 1.0,
                        })
                        .collect()
                }),
                meal: None,
                overrides: None,
                request_id: format!("synthetic-meal-{index}"),
                photo: meal.plate.map(synthetic_plate),
            },
        )
        .await
        .map_err(|error| anyhow::anyhow!("seeding a synthetic meal failed: {error}"))?;
    }
    for days_ago in 2..=56_i64 {
        if days_ago % 5 == 0 || days_ago % 11 == 0 {
            continue;
        }
        let day = (now - chrono::TimeDelta::days(days_ago)).date_naive();
        let midnight = day.and_time(chrono::NaiveTime::MIN).and_utc();
        let vary = |step: i64, spread: i64| ((days_ago * step) % spread) as f64;
        let history: [HistoryMeal; 3] = [
            HistoryMeal {
                hour: 7,
                portions: vec![
                    HistoryPortion {
                        food_id: 1693,
                        grams: 220.0 + vary(37, 80),
                    },
                    HistoryPortion {
                        food_id: 60,
                        grams: 80.0 + vary(13, 30),
                    },
                ],
            },
            HistoryMeal {
                hour: 11,
                portions: vec![
                    HistoryPortion {
                        food_id: 907,
                        grams: 180.0 + vary(23, 60),
                    },
                    HistoryPortion {
                        food_id: 300,
                        grams: 260.0 + vary(31, 90),
                    },
                ],
            },
            HistoryMeal {
                hour: 17,
                portions: vec![
                    HistoryPortion {
                        food_id: 907,
                        grams: 220.0 + vary(53, 80),
                    },
                    HistoryPortion {
                        food_id: 300,
                        grams: 300.0 + vary(29, 120),
                    },
                    HistoryPortion {
                        food_id: 3,
                        grams: 120.0 + vary(17, 60),
                    },
                ],
            },
        ];
        for (meal, HistoryMeal { hour, portions }) in history.into_iter().enumerate() {
            meal_logs::log_meal(
                db,
                foods,
                photos,
                LogMeal {
                    eaten_at: (midnight + chrono::TimeDelta::hours(hour)).to_rfc3339(),
                    note: None,
                    portions: Some(
                        portions
                            .into_iter()
                            .map(|HistoryPortion { food_id, grams }| PortionRequest {
                                food_id: Some(food_id),
                                barcode: None,
                                grams,
                                estimates: std::collections::BTreeMap::new(),
                            })
                            .collect(),
                    ),
                    supplement_doses: (meal == 0 && (17..=20).contains(&days_ago)).then(|| {
                        vec![DoseRequest {
                            supplement_id: supplement_ids[2].clone(),
                            doses: 1.0,
                        }]
                    }),
                    meal: None,
                    overrides: None,
                    request_id: format!("synthetic-history-{days_ago}-{meal}"),
                    photo: None,
                },
            )
            .await
            .map_err(|error| anyhow::anyhow!("seeding a synthetic past meal failed: {error}"))?;
        }
    }
    meals::save_meal(
        db,
        foods,
        meals::SaveMeal {
            name: "Protein oats".into(),
            portions: vec![
                meals::MealItemRequest {
                    food_id: Some(1693),
                    barcode: None,
                    grams: 150.0,
                },
                meals::MealItemRequest {
                    food_id: Some(60),
                    barcode: None,
                    grams: 50.0,
                },
            ],
            supplement_doses: vec![],
            request_id: "synthetic-meal-save".into(),
        },
    )
    .await
    .map_err(|error| anyhow::anyhow!("saving the synthetic Meal failed: {error}"))?;
    meal_logs::log_meal(
        db,
        foods,
        photos,
        LogMeal {
            eaten_at: (now - chrono::TimeDelta::hours(8)).to_rfc3339(),
            note: None,
            portions: None,
            supplement_doses: None,
            meal: Some("protein oats".into()),
            overrides: Some(vec![OverrideRequest {
                food_id: Some(60),
                barcode: None,
                grams: Some(70.0),
                supplement_id: None,
                doses: None,
            }]),
            request_id: "synthetic-meal-by-name".into(),
            photo: None,
        },
    )
    .await
    .map_err(|error| anyhow::anyhow!("logging the synthetic Meal failed: {error}"))?;
    for food_id in [3, 60, 907, 1693] {
        nova::classify_food(db, foods, food_id, nova::ClassifyFood { nova_class: 1 })
            .await
            .map_err(|error| anyhow::anyhow!("classifying a synthetic Food failed: {error}"))?;
    }
    Ok(())
}
