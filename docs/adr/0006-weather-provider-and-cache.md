# Weather provider and cache belong to Axum

Weather is a new integration capability. Existing IR, SwitchBot, AdGuard and
Docker services were considered: none owns forecast data, geographic
configuration or provider caching, so extending one would couple unrelated
integrations. A dedicated WeatherService owns the Open-Meteo HTTP adapter,
normalization and shared in-memory cache. Its route owns this state without
adding a database dependency. Dashboard server functions reuse the existing
private OpenHome transport.

Coordinates are validated at startup and kept in private API configuration.
Responses expose no coordinates or upstream errors. Dates use Europe/Copenhagen;
timestamps are UTC instants and units are Celsius, m/s and mm. Missing values
remain null. A successful forecast is fresh for 15 minutes; failures retry no
more than once per minute and may serve explicitly stale data for up to six
hours. Concurrent requests share a refresh; upstream calls time out after five
seconds. Cache loss on restart is acceptable for this read-only integration.

Open-Meteo's [terms](https://open-meteo.com/en/terms), checked 2026-10-11,
explicitly permit personal home automation on the free non-commercial API.
One active instance uses at most 96 successful refreshes per day, or 1,440
attempts during a continuous outage, within the stated limits. The Weather
view attributes Open-Meteo and its CC BY 4.0 license. Commercial hosting would
require reassessing those terms. No paid plan or provider credentials are used.
