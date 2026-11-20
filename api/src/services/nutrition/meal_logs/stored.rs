use super::{
    KeyNutrients, MealLog, PHOTO_PATH, Photo, PhotoThumbnail, Portion, PortionAmount,
    PortionProduct, Result, Total, ValueSource,
};
use crate::services::nutrition::intake::Intake;
use crate::services::nutrition::meal_photos::Size;
use crate::services::nutrition::nova::NovaClass;
use crate::services::nutrition::parameters::{self, Unit};
use crate::services::nutrition::supplements::StoredDose;
use crate::services::training::calendar;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug)]
pub(in crate::services::nutrition) struct StoredValue {
    pub value: f64,
    pub unit: String,
    pub source: ValueSource,
}

#[derive(Debug)]
pub(in crate::services::nutrition) struct StoredPortion {
    pub(super) food_id: i64,
    pub(super) product: Option<PortionProduct>,
    pub(super) name_da: String,
    pub(super) name_en: String,
    pub grams: f64,
    pub(super) dataset_version: String,
    pub nova_class: Option<NovaClass>,
    pub values: HashMap<String, StoredValue>,
}

impl StoredPortion {
    pub(in crate::services::nutrition) fn dataset_version(&self) -> &str {
        &self.dataset_version
    }

    pub(in crate::services::nutrition) fn is_product(&self) -> bool {
        self.product.is_some()
    }

    pub(in crate::services::nutrition) fn amount(&self, key: &str) -> Option<f64> {
        self.values
            .get(key)
            .map(|stored| stored.value * self.grams / 100.0)
    }
}

#[derive(Debug)]
pub(super) struct StoredPhoto {
    pub(super) id: String,
    pub(super) size: Size,
    pub(super) thumbnail_size: Size,
}

impl StoredPhoto {
    pub(super) fn from_columns(
        id: Option<String>,
        width: Option<i64>,
        height: Option<i64>,
        thumbnail_width: Option<i64>,
        thumbnail_height: Option<i64>,
    ) -> Option<Self> {
        let side = |value: Option<i64>| value.and_then(|value| u32::try_from(value).ok());
        Some(Self {
            id: id?,
            size: Size {
                width: side(width)?,
                height: side(height)?,
            },
            thumbnail_size: Size {
                width: side(thumbnail_width)?,
                height: side(thumbnail_height)?,
            },
        })
    }

    fn into_photo(self) -> Photo {
        let url = format!("{PHOTO_PATH}/{}", self.id);
        Photo {
            thumbnail: PhotoThumbnail {
                width: self.thumbnail_size.width,
                height: self.thumbnail_size.height,
                url: format!("{url}/thumbnail"),
            },
            id: self.id,
            width: self.size.width,
            height: self.size.height,
            url,
        }
    }
}

#[derive(Debug)]
pub(in crate::services::nutrition) struct StoredLog {
    pub(super) id: String,
    pub(super) eaten_at: String,
    pub day: String,
    pub(super) meal_name: Option<String>,
    pub(super) note: Option<String>,
    pub(super) photo: Option<StoredPhoto>,
    pub portions: Vec<StoredPortion>,
    pub doses: Vec<StoredDose>,
}

pub(in crate::services::nutrition) fn round(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

pub(super) fn vocabulary_unit(key: &str) -> Option<&'static str> {
    parameters::by_key(key).and_then(|parameter| Unit::symbol(parameter.unit))
}

pub(in crate::services::nutrition) fn key_totals(
    portions: &[&StoredPortion],
    doses: &[&StoredDose],
) -> KeyNutrients<Total> {
    KeyNutrients::from_fn(|key| {
        Intake::of(key, portions, doses).into_total(vocabulary_unit(key).map(str::to_owned))
    })
}

impl StoredLog {
    pub(in crate::services::nutrition) fn into_meal_log(self) -> Result<MealLog> {
        let eaten_at = DateTime::parse_from_rfc3339(&self.eaten_at)
            .map_err(|error| sqlx::Error::Decode(Box::new(error)))?
            .with_timezone(&Utc);
        let totals = key_totals(
            &self.portions.iter().collect::<Vec<_>>(),
            &self.doses.iter().collect::<Vec<_>>(),
        );
        Ok(MealLog {
            id: self.id,
            eaten_at: calendar::iso(eaten_at),
            day: self.day,
            meal_name: self.meal_name,
            note: self.note,
            photo: self.photo.map(StoredPhoto::into_photo),
            portions: self
                .portions
                .into_iter()
                .map(|portion| Portion {
                    nutrients: KeyNutrients::from_fn(|key| {
                        let stored = portion.values.get(key);
                        PortionAmount {
                            value: portion.amount(key).map(round),
                            unit: vocabulary_unit(key).map(str::to_owned),
                            source: stored.map(|stored| stored.source),
                        }
                    }),
                    food_id: portion.food_id,
                    product: portion.product,
                    name_da: portion.name_da,
                    name_en: portion.name_en,
                    grams: portion.grams,
                    dataset_version: portion.dataset_version,
                    nova_class: portion.nova_class,
                })
                .collect(),
            supplement_doses: self.doses.iter().map(StoredDose::to_dose).collect(),
            totals,
        })
    }
}
