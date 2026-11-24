use crate::send_request_with_method::send_request_with_method;
use crate::support::{Nutrition, at, breakfast, synthetic, with_photo};
use axum::{http::Method, http::StatusCode};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[tokio::test]
async fn a_photo_is_stored_upright_without_metadata_as_avif_with_a_thumbnail() {
    let api = Nutrition::start().await;
    let jpeg = synthetic::jpeg_with_gps(2400, 1200, 6);
    assert!(synthetic::contains(&jpeg, synthetic::CAMERA));

    let meal_log = api.logged(with_photo(breakfast("photo-1"), &jpeg)).await;

    let photo = &meal_log["photo"];
    let id = photo["id"].as_str().unwrap();
    assert_eq!(
        *photo,
        json!({
            "id": id,
            "width": 800,
            "height": 1600,
            "url": format!("/api/nutrition/meal-photos/{id}"),
            "thumbnail": {
                "width": 200,
                "height": 400,
                "url": format!("/api/nutrition/meal-photos/{id}/thumbnail"),
            },
        })
    );
    for (url, size, top, bottom) in [
        (&photo["url"], (800, 1600), (400, 200), (400, 1400)),
        (
            &photo["thumbnail"]["url"],
            (200, 400),
            (100, 50),
            (100, 350),
        ),
    ] {
        let (status, content_type, avif) = api.media(url.as_str().unwrap()).await;
        assert_eq!(
            (status, content_type.as_deref()),
            (StatusCode::OK, Some("image/avif"))
        );
        assert!(!synthetic::contains(&avif, b"Exif"), "{url}");
        assert!(!synthetic::contains(&avif, synthetic::CAMERA), "{url}");
        let (decoded, (red, _, blue)) = synthetic::avif_pixel(&avif, top.0, top.1);
        assert_eq!(decoded, size, "{url}");
        assert!(red > 180 && blue < 80, "top of {url} is ({red}, {blue})");
        let (_, (red, _, blue)) = synthetic::avif_pixel(&avif, bottom.0, bottom.1);
        assert!(blue > 180 && red < 80, "bottom of {url} is ({red}, {blue})");
    }
    assert_eq!(
        api.photo_files(),
        [format!("{id}.avif"), format!("{id}.thumbnail.avif")]
    );
    assert_eq!(api.day("2026-10-29").await, std::slice::from_ref(&meal_log));
    let (status, nutrition_day) = api.get("/api/nutrition/days/2026-10-29").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(nutrition_day["meal_logs"], json!([meal_log]));
}

#[tokio::test]
async fn photos_are_scaled_down_to_their_long_side_and_never_up() {
    let api = Nutrition::start().await;
    for (index, (width, height, photo, thumbnail)) in [
        (3200, 800, (1600, 400), (400, 100)),
        (1000, 800, (1000, 800), (400, 320)),
        (300, 200, (300, 200), (300, 200)),
    ]
    .into_iter()
    .enumerate()
    {
        let mut body = at("2026-10-29T12:00:00+01:00", &format!("scaled-{index}"));
        body["photo"] = json!(format!(
            "data:image/png;base64,{}",
            synthetic::base64(&synthetic::png(width, height))
        ));
        let logged = api.logged(body).await["photo"].clone();
        assert_eq!(
            (
                (&logged["width"], &logged["height"]),
                (
                    &logged["thumbnail"]["width"],
                    &logged["thumbnail"]["height"]
                )
            ),
            (
                (&json!(photo.0), &json!(photo.1)),
                (&json!(thumbnail.0), &json!(thumbnail.1))
            ),
            "{width}x{height}"
        );
        let (_, _, avif) = api.media(logged["url"].as_str().unwrap()).await;
        assert_eq!(synthetic::avif_pixel(&avif, 0, 0).0, photo);
    }
}

#[tokio::test]
async fn an_undecodable_or_oversized_photo_rejects_the_whole_request_and_writes_nothing() {
    let api = Nutrition::start().await;
    let existing = api
        .logged(at("2026-10-28T12:00:00+01:00", "existing"))
        .await;
    let existing_uri = format!(
        "/api/nutrition/meal-logs/{}",
        existing["id"].as_str().unwrap()
    );
    let jpeg_start = |length: usize| {
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0];
        bytes.resize(length, 0x42);
        bytes
    };
    let ten_mb = 10 * 1024 * 1024;
    let cases: [(&str, Value, StatusCode); 7] = [
        ("not base64", json!("not base64!"), StatusCode::BAD_REQUEST),
        (
            "a GIF",
            json!(synthetic::base64(b"GIF89a\x01\x00\x01\x00")),
            StatusCode::BAD_REQUEST,
        ),
        (
            "a broken JPEG",
            json!(synthetic::base64(&jpeg_start(2_000))),
            StatusCode::BAD_REQUEST,
        ),
        ("not a string", json!(42), StatusCode::BAD_REQUEST),
        (
            "wider than 12000 px",
            json!(synthetic::base64(&synthetic::png(12_001, 1))),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
        (
            "over 10 MB, inside the body limit",
            json!(synthetic::base64(&jpeg_start(ten_mb + 1))),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
        (
            "over the 15 MB body limit",
            json!(synthetic::base64(&jpeg_start(12 * 1024 * 1024))),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
    ];
    for (case, photo, expected) in cases {
        let mut body = at("2026-10-29T12:00:00+01:00", "rejected");
        body["photo"] = photo.clone();
        let (status, response) = api.log(body).await;
        assert_eq!(status, expected, "log_meal with {case}: {response}");
        assert_eq!(response["status"], expected.as_u16(), "{case}: {response}");

        let (status, response) = api
            .send(
                Method::PATCH,
                &existing_uri,
                Some(json!({"note": "changed", "photo": photo})),
            )
            .await;
        assert_eq!(status, expected, "update with {case}: {response}");
    }
    let (status, response) = api
        .upload(
            existing["id"].as_str().unwrap(),
            "photo",
            &jpeg_start(2_000),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
    let (status, response) = api
        .upload(
            existing["id"].as_str().unwrap(),
            "picture",
            &synthetic::png(10, 10),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "no photo field: {response}"
    );

    assert_eq!(api.day("2026-10-29").await, Vec::<Value>::new());
    assert_eq!(api.day("2026-10-28").await, [existing]);
    assert_eq!(api.photo_files(), Vec::<String>::new());
    let logged = api
        .logged(with_photo(
            at("2026-10-29T12:00:00+01:00", "rejected"),
            &synthetic::png(20, 10),
        ))
        .await;
    assert_eq!(logged["photo"]["width"], 20);
}

const PATCH_SIZE: u32 = 32;
const PATCH_CENTER: u32 = PATCH_SIZE / 2;
const DISPLAY_P3_PATCH: [u8; 3] = [128, 200, 64];
const SRGB_OF_DISPLAY_P3_PATCH: [u8; 3] = [103, 202, 19];
const COLOUR_TOLERANCE: i16 = 6;
const SRGB_NCLX_PARAMETERS: [u8; 7] = [0, 1, 0, 13, 0, 6, 128];

#[tokio::test]
async fn wide_gamut_photo_pixels_are_normalized_and_tagged_srgb_without_retaining_the_profile() {
    use image::{ImageEncoder, codecs::png::PngEncoder};
    let api = Nutrition::start().await;
    let pixels = image::RgbImage::from_pixel(PATCH_SIZE, PATCH_SIZE, image::Rgb(DISPLAY_P3_PATCH));
    let profile = moxcms::ColorProfile::new_display_p3().encode().unwrap();
    let mut png = Vec::new();
    let mut encoder = PngEncoder::new(&mut png);
    encoder.set_icc_profile(profile).unwrap();
    encoder
        .write_image(
            pixels.as_raw(),
            PATCH_SIZE,
            PATCH_SIZE,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    let log = api.logged(with_photo(breakfast("wide-gamut"), &png)).await;
    for url in [&log["photo"]["url"], &log["photo"]["thumbnail"]["url"]] {
        let (_, _, avif) = api.media(url.as_str().unwrap()).await;
        let (_, (r, g, b)) = synthetic::avif_pixel(&avif, PATCH_CENTER, PATCH_CENTER);
        let decoded = [r, g, b];
        assert!(
            decoded
                .iter()
                .zip(SRGB_OF_DISPLAY_P3_PATCH)
                .all(
                    |(actual, expected)| (i16::from(*actual) - i16::from(expected)).abs()
                        < COLOUR_TOLERANCE
                ),
            "decoded sRGB patch: {decoded:?}"
        );
        let colr = avif
            .windows(8)
            .position(|bytes| bytes == b"colrnclx")
            .unwrap()
            + 8;
        assert_eq!(&avif[colr..colr + 7], &SRGB_NCLX_PARAMETERS);
        assert!(!avif.windows(4).any(|bytes| bytes == b"prof"));
    }
}

#[tokio::test]
async fn photo_media_requires_the_api_key() {
    let api = Nutrition::start().await;
    let meal_log = api
        .logged(with_photo(breakfast("photo-auth"), &synthetic::png(40, 30)))
        .await;
    let (status, _) = send_request_with_method(
        api.app.clone(),
        meal_log["photo"]["url"].as_str().unwrap(),
        Method::GET,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
