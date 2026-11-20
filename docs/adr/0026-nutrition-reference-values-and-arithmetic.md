# Nutrition reference values and arithmetic

`docs/context/nutrition.md` defines the nutrition terms, and ADRs 0012–0014
fix how values are snapshotted, imported and frozen. This record keeps the
sources, constants and normalisation choices behind those terms, which the code
cannot justify by itself.

- **Hand-transcribed references.** Neither NNR 2023 nor EFSA publishes
  machine-readable tables, so `micronutrients/references.rs` is transcribed by
  hand. Recommended and adequate intakes come from NNR 2023 (Nord 2023:003,
  with the 25 September 2023 corrections), RI Tables 12/14 and AI Tables 13/15,
  for adult age bands 18–24, 25–50, 51–70 and 71+. Thiamin and niacin per-MJ
  RIs are converted with Table 8 reference energy at PAL 1.6, the level NNR
  uses for its per-day figures. Upper Limits are EFSA's TUL overview v11
  (August 2025), Tables 1 and 3, adult column. EPA+DHA uses EFSA NDA 2010's
  adequate intake of 250 mg/day, because NNR sets none.
- **Footnoted rows.** Vitamin D is 20 µg over 70 and for little sun exposure
  (NNR footnote 3). Females aged 51–70 who still menstruate use 15 mg iron
  (footnote 7). Selected rows cite the alternate NNR or EFSA values recorded
  beside them in the table.
- **Units follow the food database.** Every reference amount is in the unit of
  the database parameter it is compared with: copper in mg, molybdenum and
  fluoride in µg, and EPA+DHA in mg even though it is stored in grams.
- **Deliberate omissions.** Iron, manganese and fluoride have no Upper Limit,
  because EFSA gives only safe levels that it says cannot judge risk. Sodium is
  omitted: its AI of 1.5 g would read as "reaching it is good", while NNR
  recommends limiting intake to 2.3 g. Boron has no NNR reference, and chromium
  has neither reference. Compounds without a database parameter have no
  reference.
- **Upper Limits by form.** Some limits cover only part of intake. Magnesium's
  covers supplements only (dissociable salts and oxide). Retinol's covers
  retinol and retinyl esters but not provitamin A carotenoids. Folic acid and
  L-methylfolate are limited apart from natural folate, and nicotinic acid and
  nicotinamide apart from each other. Intake therefore keeps food and
  supplement shares separate, and a Supplement amount must name its chemical
  form. The retinol share counts retinol in food, retinol taken as vitamin A,
  and retinol listed as itself.
- **Why 70 % for a Lasting Shortfall.** An NNR 2023 RI sits about two standard
  deviations above the average requirement. Below 70 % of RI is therefore
  below what most people need, while anything between that and RI is not.
- **Range arithmetic.** Ranges sum values first and then divide, and never
  average daily shares, so any span's share follows from its sums. The same
  applies to the NOVA 1 share, which is kcal of Class 1 over kcal of classified
  Portions. Compounds reported in several mass units sum in the unit first
  taken.
- **Target engine constants.** These live in `targets.rs`. Trend Weight is an
  EMA with factor 0.1. 7,700 kcal is used per kg of Trend Weight change. The
  prior is Mifflin-St Jeor BMR × 1.5, blended out at min(logged days, 14)/14.
  Protein is 2.2 g/kg, and the fat floor is 0.7 g/kg. Carbs take the remaining
  energy at 4/4/9 kcal/g and never go below zero. The Profile stores sex as
  free text, so only `male` or `female` (any case) is accepted and nothing else
  is guessed.
- **Open Food Facts labels.** OFF stores every `_100g` mass in grams (vitamin D
  1.5 µg is `0.0000015`). Values are converted to vocabulary units and rounded
  to six significant digits to remove float noise. Carbohydrate is available
  carbohydrate, with fibre declared separately. Niacin is niacin equivalents.
  Salt is sodium × 2.5. Alcohol % vol × 0.789 gives grams per 100 ml, which is
  treated as per 100 g for drinks near water density. Negative values, or
  values over 100 g per 100 g, are dropped as unknown.
- **DTU workbook import.** Columns are found by header name, so added or
  reordered columns still import. An empty `Data_Table` cell is unknown, while
  a recorded zero stays zero. ParameterIDs run from the FoodID marker to the
  trailing food-group columns. DTU renumbers Foods between versions but keeps
  ParameterIDs, so each ParameterID gets a stable key and unit. A workbook whose
  unit changes for a known parameter is refused, so a key never silently
  changes meaning. A parameter added after the vocabulary still imports under a
  DTU-derived key. Waste and density describe the Food and are not per-100 g
  amounts. Glycerol is defined on the Parameter sheet but has no data column.
- **Danish name search.** DTU names are comma-separated parts, while people
  type compounds. A query word matches when it is one of the name's words,
  when it appears in the name with spaces removed, or, at 5+ letters, when most
  of its letter triples occur. Exact and first-component names rank first, so
  "Tomato, raw" comes before "Tomato juice". Cooked porridge never aliases to
  "Oats, rolled", because water changes the per-100 g values.
- **Barcodes.** Barcodes are canonicalised to EAN-8 or GTIN-13, so one package
  is one Product: UPC-A gains a leading zero and a zero-led GTIN-14 loses it.
  The GS1 mod-10 check must pass, except that 8 digits may also be UPC-E.
- **Energy bound.** A portion's energy estimate is capped at pure fat, the
  most 100 g can hold. Past eaten-at times are backfills. A slightly future
  time is tolerated for client clock drift, and anything further is refused.
