#let yellow = rgb("F5C451")
#let background = rgb("08090D")
#let panel = rgb("F1F2F4")
#let muted = rgb("596170")
#let green = rgb("247B4B")

#set document(
  title: "OpenHome - projektdokumentation",
  author: "OpenHome",
)
#set page(
  paper: "a4",
  margin: (x: 20mm, y: 18mm),
  numbering: "1 / 1",
  footer: context [
    #set text(size: 8pt, fill: muted)
    OpenHome #h(1fr) Projektdokumentation #h(1fr) #counter(page).display()
  ],
)
#set text(font: "DejaVu Sans", lang: "da", size: 10pt, fill: background)
#set par(leading: 0.65em)
#set heading(numbering: "1.1")
#show heading.where(level: 1): it => {
  v(0.8em)
  text(size: 18pt, weight: "bold", fill: background, it.body)
  v(0.25em)
  line(length: 100%, stroke: 1.5pt + yellow)
  v(0.35em)
}
#show heading.where(level: 2): it => {
  v(0.45em)
  text(size: 12pt, weight: "bold", fill: background, it.body)
  v(0.1em)
}
#show strong: set text(fill: background)

#let status(label, color: green) = box(
  inset: (x: 6pt, y: 3pt),
  radius: 4pt,
  fill: color.lighten(82%),
  stroke: 0.6pt + color,
  text(size: 8pt, weight: "bold", fill: color.darken(20%), label),
)

#let requirement(number, title, body) = block(
  width: 100%,
  inset: 10pt,
  radius: 6pt,
  fill: panel,
  stroke: 0.5pt + rgb("D7DAE0"),
  [
    #grid(
      columns: (28pt, 1fr),
      gutter: 7pt,
      align: (center, left),
      circle(radius: 12pt, fill: yellow)[#text(weight: "bold", str(number))],
      [*#title*],
    )
    #v(4pt)
    #body
  ],
)

#align(center)[
  #v(18mm)
  #text(size: 12pt, weight: "bold", tracking: 1.5pt, fill: yellow)[CROSSPLATFORM MOBILAPPLIKATION]
  #v(5mm)
  #text(size: 34pt, weight: "bold")[OpenHome]
  #v(2mm)
  #text(size: 18pt, fill: muted)[Projektdokumentation]
  #v(10mm)
  #line(length: 45mm, stroke: 3pt + yellow)
  #v(10mm)
  #text(size: 12pt)[React Native · Expo · TypeScript · Axum REST API]
  #v(3mm)
  #text(fill: muted)[Android og iOS]
]

#v(1fr)
#block(
  inset: 12pt,
  radius: 7pt,
  fill: background,
  width: 100%,
)[
  #set text(fill: white)
  *Formål*\
  OpenHome er en mobil fjernbetjening til et privat smart home. Appen styrer lys, TV og højttaler gennem en ekstern Axum API og kan automatisk slukke lyset, når brugeren forlader hjemmet.
]

#pagebreak()

= Projekt og løsning

OpenHome samler de mest brugte funktioner fra hjemmet i én app. Brugeren konfigurerer serverens adresse og en API-nøgle, hvorefter appen kan sende kommandoer til lys, TV og højttaler. Axum API'en videresender HTTP-requests til to ESP32-enheder: én styrer TV og højttaler med infrarøde signaler, mens den anden styrer en SwitchBot, der tænder og slukker lyset. En valgfri geofence registrerer, når telefonen forlader hjemmet, og sender automatisk en kommando om at slukke lyset.

#grid(
  columns: (1fr, 1fr, 1fr),
  gutter: 8pt,
  block(fill: panel, inset: 9pt, radius: 5pt)[*Klient* #linebreak() React Native og Expo],
  block(fill: panel, inset: 9pt, radius: 5pt)[*Backend* #linebreak() Rust og Axum],
  block(fill: panel, inset: 9pt, radius: 5pt)[*Kommunikation* #linebreak() REST og JSON],
)

== Clean Architecture

Projektet anvender en Clean Architecture-inspireret lagdeling og er opdelt efter ansvar:

- `domain`: datatyper, validering og rene regler.
- `application`: tilstande og koordinering af brugerhandlinger.
- `infrastructure`: REST API, SecureStore, GPS og geofencing.
- `ui`: skærmbilleder og genbrugelige React Native-komponenter.

Denne opdeling gør domænelogikken testbar og holder platforms- og netværkskode væk fra brugerfladen. Afhængigheder abstraheres gennem interfaces, så eksempelvis lagring og geofencing kan udskiftes uden at ændre domænereglerne.

= Obligatoriske krav

#requirement(1, "Eksterne data og gemt konfiguration", [
  Appen henter IR-status og sender kommandoer til Axum API'en. Base URL, API-nøgle, hjemmeposition og ventende baggrundskommandoer gemmes krypteret med Expo SecureStore og genindlæses mellem sessioner.
])

#v(6pt)
#requirement(2, "Selvstændig udvikling", [
  Projektet er udviklet i små, selvstændige dele: opsætning, API-styring, geofence, native Android-modul og swipe-navigation. Den Clean Architecture-inspirerede struktur adskiller domæne, applikation, infrastruktur og UI, så hver del har ét tydeligt ansvar.
])

#v(6pt)
#requirement(3, "Valg af udviklingsværktøj", [
  React Native med Expo blev valgt, fordi én TypeScript-kodebase kan bruges på Android og iOS. Expo tilbyder gennemprøvede API'er til placering, sikker lagring og baggrundsopgaver. React Native bruger native UI-komponenter, og Expo kan udvides med native Kotlin-kode, når standardmodulerne ikke er nok.
])

#v(6pt)
#requirement(4, "Ekstern datakilde", [
  Appen kommunikerer med en ekstern REST API via `fetch`. Den læser dynamisk tilgængelige IR-kommandoer og sender POST-kald til TV, højttaler og lys. Axum API'en sender derefter HTTP-requests til to ESP32-enheder: en infrarød fjernbetjening til TV og højttaler samt en SwitchBot-controller til lyset. JSON-svar behandles som ukendte data og valideres før brug.
])

#v(6pt)
#requirement(5, "Samspil med enheden", [
  Appen anmoder om præcis GPS-position og tilladelse til baggrundsplacering. Positionen bruges til at oprette en Home Geofence. Android har desuden et lokalt Kotlin-modul baseret på `LocationManager`; iOS anvender Expo Location.
])

#v(6pt)
#requirement(6, "Asynkrone processer", [
  Netværk, sikker lagring, GPS og baggrundsopgaver anvender Promises samt `async`/`await`. Det forhindrer langsomme operationer i at blokere brugerfladen. Samtidige kommandoer følges uafhængigt, så eksempelvis TV og højttaler kan styres uden at låse hele appen.
])

#pagebreak()

#requirement(7, "Optimering og ressourceforbrug", [
  Geofencing udføres af operativsystemet i stedet for kontinuerlig GPS-polling. API-status hentes ved opstart og ved et aktivt genforsøg, ikke løbende. Dublerede knaptryk afvises, requests afbrydes efter fem sekunder, og skærmbilleder deler samme indlæste IR-status.
])

#v(6pt)
#requirement(8, "Sikkerhed", [
  Alle API-kald kræver en Bearer API-nøgle. Nøglen vises skjult og gemmes i platformens SecureStore. URL, nøgle, API-svar, koordinater og radius valideres. HTTPS bør bruges uden for et betroet lokalnet; den nuværende udviklingsopsætning tillader også HTTP til en lokal server og skal derfor ikke bruges over et ubeskyttet netværk.
])

#v(6pt)
#requirement(9, "Dokumentation og test", [
  Projektet har 24 automatiske tests fordelt på seks testfiler. De dækker validering, state transitions, API-svar, geofence, fejlhåndtering og retry. TypeScript typecheck, Expo Doctor samt Android- og iOS-bundling er bestået. Screenshots er vist i dette dokument. Demo-video, manuel testrapport og brugerfeedback vedlægges afleveringen.
])

#v(6pt)
#requirement(10, "Mundtlig præsentation", [
  Præsentationen planlægges til 10-15 minutter: problem og målgruppe, live demo, arkitektur, API og sikkerhed, geofencing, test, udfordringer og refleksion. Demoen viser opsætning, fjernbetjening og Leave home-automatik.
])

#v(6pt)
#requirement(11, "Problemløsning og refleksion", [
  Expo-geofencing på Android afhænger normalt af Google Play Services. For at understøtte enheder uden Google Play blev der udviklet et Kotlin-modul med Androids `LocationManager`. Modulet genbruger samme TypeScript-applikationslogik. Løsningen øger kompleksiteten, men undgår løbende GPS-opdateringer og bevarer lavt batteriforbrug.
])

#v(6pt)
#requirement(12, "Metodevalg", [
  Arbejdet fulgte en enkel, Kanban-inspireret og iterativ metode. Funktioner blev opdelt i små vertikale leverancer, implementeret én ad gangen og kontrolleret med tests og builds. Metoden passede til et enkeltmandsprojekt, fordi prioriteringer kunne ændres uden faste sprintceremonier.
])

= Valgfri funktionalitet

Følgende valgfri funktioner er implementeret:

- *Dynamiske lister og brugerinput:* API'en bestemmer, hvilke fjernbetjeningskommandoer der er tilgængelige. Brugeren indtaster server, API-nøgle og radius.
- *Lokal lagring:* SecureStore gemmer konfiguration og geofence krypteret.
- *Touch og interaktivitet:* knapper, switch, faner og vandret swipe-navigation.
- *Sensorer:* GPS og geofencing bruges til Leave home-automatik.
- *Udvidet API-integration:* dynamiske JSON-data og Bearer-authentication.

= Skærmbilleder

#grid(
  columns: (1fr, 1fr),
  gutter: 10pt,
  figure(image("home.png", width: 65%), caption: [Home med hurtig adgang]),
  figure(image("away.png", width: 65%), caption: [GPS-baseret automatik]),
  figure(image("tv.png", width: 65%), caption: [Dynamisk TV-fjernbetjening]),
  figure(image("speaker.png", width: 65%), caption: [Højttalerstyring]),
)

= Kort testrapport

#table(
  columns: (1.25fr, 2fr, 0.8fr),
  inset: 7pt,
  stroke: 0.5pt + rgb("D7DAE0"),
  fill: (x, y) => if y == 0 { background } else if calc.even(y) { panel } else { white },
  text(fill: white, weight: "bold", [Test]),
  text(fill: white, weight: "bold", [Formål]),
  text(fill: white, weight: "bold", [Resultat]),
  [TypeScript], [Kontrol af typer og interfaces], [#status("Verificeret")],
  [Vitest], [24 tests i seks testfiler], [#status("Verificeret")],
  [Expo Doctor], [20 projekt- og afhængighedstjek], [#status("Verificeret")],
  [Android export], [JavaScript-bundle til Android], [#status("Verificeret")],
  [iOS export], [JavaScript-bundle til iOS], [#status("Verificeret")],
  [Fysisk enhed], [Tilladelser, GPS, baggrund og API], [#status("Verificeret")],
)

== Forslag til manuel brugertest

1. Gem en gyldig serveradresse og API-nøgle, genstart appen, og kontrollér at opsætningen huskes.
2. Send kommandoer til TV, højttaler og lys, og kontrollér den fysiske reaktion.
3. Afvis og godkend placeringstilladelser, og kontrollér fejlbeskederne.
4. Opret en Home Geofence, forlad området, og kontrollér at lyset slukkes.
5. Afbryd netværket, udløs en kommando, og kontrollér fejl samt retry efter genåbning.

= Præsentationsplan

#table(
  columns: (2.2fr, 0.7fr),
  inset: 6pt,
  stroke: 0.5pt + rgb("D7DAE0"),
  [*Indhold*], [*Tid*],
  [Idé, målgruppe og problem], [1 min.],
  [Live demo af de vigtigste funktioner], [4 min.],
  [Arkitektur, API og asynkron kode], [3 min.],
  [GPS, geofencing, optimering og sikkerhed], [3 min.],
  [Test, udfordringer og refleksion], [2 min.],
  [Spørgsmål], [2 min.],
)

== Afleveringstjek

- #status("Klar") Kildekode og screenshots.
- #status("Klar") Automatisk test, typecheck og platform-bundles.
- #status("Klar") Projektdokumentation og præsentationsplan.
