use crate::builder::{MACRO, SyntheticFood, SyntheticParameter, parameter, workbook};

pub const FACTORS: (&str, &str) = ("Faktorer m.m.", "Factors etc.");

pub fn parameters(protein_unit: &'static str) -> Vec<SyntheticParameter> {
    vec![
        parameter(137, ("Energi (kJ)", "Energy (kJ)"), "kJ/100g", 1010, MACRO),
        parameter(
            356,
            ("Energi (kcal)", "Energy (kcal)"),
            "kcal/100 g",
            1030,
            MACRO,
        ),
        parameter(218, ("Protein", "Protein"), protein_unit, 1110, MACRO),
        parameter(
            172,
            ("Tilgængelig kulhydrat", "Available carbohydrates"),
            "g/100g",
            1220,
            MACRO,
        ),
        parameter(168, ("Kostfibre", "Dietary fibre"), "g/100g", 1240, MACRO),
        parameter(141, ("Fedt", "Fat"), "g/100g", 1310, MACRO),
        parameter(19, ("Alkohol", "Alcohol"), "g/100g", 1410, MACRO),
        parameter(
            126,
            ("D-vitamin", "Vitamin D"),
            "µg/100g",
            2110,
            ("Vitaminer", "Vitamins"),
        ),
        SyntheticParameter {
            has_column: false,
            ..parameter(
                447,
                ("Glycerin", "Glycerol"),
                "g/100g",
                4580,
                ("Sukkeralkoholer", "Sugar alcohols"),
            )
        },
        parameter(
            87,
            ("C20:5,n-3", "C20:5,n-3"),
            "g/100g",
            9110,
            ("Flerumættede fedtsyrer", "Polyunsaturated fatty acids"),
        ),
        parameter(252, ("Svind", "Waste"), "%", 20010, FACTORS),
        parameter(363, ("Massefylde", "Density"), "g/ml", 20210, FACTORS),
        parameter(432, ("pH", "pH"), "No Unit", 20220, FACTORS),
    ]
}

pub fn skyr(protein: f64) -> SyntheticFood {
    SyntheticFood {
        id: 1693,
        name_da: "Skyr, 0.2 % fedt",
        name_en: "Skyr, 0.2 % fat",
        group: (3, "Syrnede mælkeprodukter", "Fermented milk products"),
        values: vec![
            (137, 260.0),
            (356, 62.0),
            (218, protein),
            (172, 4.0),
            (168, 0.0),
            (141, 0.2),
            (19, 0.0),
            (126, 0.0),
            (252, 0.0),
            (363, 1.03),
            (432, 4.2),
        ],
    }
}

pub fn chicken_breast() -> SyntheticFood {
    SyntheticFood {
        id: 907,
        name_da: "Kylling, bryst, kød og skind, rå",
        name_en: "Chicken, breast, flesh and skin, raw",
        group: (80, "Fjerkræ", "Poultry"),
        values: vec![
            (356, 150.0),
            (218, 21.0),
            (141, 7.0),
            (87, 0.01),
            (252, 10.0),
        ],
    }
}

pub fn banana() -> SyntheticFood {
    SyntheticFood {
        id: 3,
        name_da: "Banan, rå",
        name_en: "Banana, raw",
        group: (52, "Sydfrugt", "Tropical or subtropical fruit"),
        values: vec![(356, 90.0), (172, 20.0), (252, 40.0)],
    }
}

pub fn apple() -> SyntheticFood {
    SyntheticFood {
        id: 2,
        name_da: "Æble, uspec., råt",
        name_en: "Apple, raw, all varieties",
        group: (49, "Kernefrugt", "Pome fruit"),
        values: vec![(356, 50.0), (172, 11.0), (252, 12.0)],
    }
}

pub fn first_dataset() -> Vec<u8> {
    workbook(
        &parameters("g/100g"),
        &[apple(), banana(), chicken_breast(), skyr(11.0)],
    )
}
