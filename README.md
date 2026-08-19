# LandEX

LandEX is a global real-estate investment terminal where people can discover and analyze properties, learn investment fundamentals, and build experience through simulated investing.

## Technology

- Backend: Rust, Actix Web, and PostgreSQL
- Web: React and TypeScript
- Mobile: Flutter

## Backend development

The backend lives in `backend/`.

1. Copy `backend/.env.example` to `backend/.env`.
2. Set `DATABASE_URL` to a PostgreSQL database.
3. Run `cargo run` from `backend/`.

For local development, PostgreSQL 16 can be started from the repository root with:

```sh
docker compose up -d postgres
```

The API verifies its database connection and applies pending migrations before it starts accepting requests.

The initial service endpoints are:

- `GET /api/v1/health` — confirms the API process is running.
- `GET /api/v1/ready` — confirms the API can reach PostgreSQL.
- `GET /api/v1/markets` — searches normalized real-estate markets and their latest metrics.
- `GET /api/v1/markets/{id}` — returns a market and its metric history.
- `GET /api/v1/locations` — searches normalized global locations.
- `GET /api/v1/locations/{id}` — returns a location and its geographic hierarchy.
- `GET /api/v1/properties` — searches active normalized listings.
- `GET /api/v1/properties/{id}` — returns the latest active listing for a property.
- `GET /api/v1/properties/{id}/history` — returns property price, rent, valuation, and market-time observations.
- `POST /api/v1/analysis/investment` — calculates explainable property investment metrics and projections.

Property search supports `country_code`, `location_id`, `property_type`, `listing_type`, `min_price`, `max_price`, `currency`, `limit`, and `offset` query parameters.

Market search supports `country_code`, `location_id`, `property_type`, `currency`, `limit`, and `offset`. Market detail accepts `history_limit`.

### RentCast ingestion

The RentCast adapter imports US sale and long-term rental listings into the canonical LandEX schema. Configure `RENTCAST_API_KEY` and one geographic scope in `backend/.env`: a state, a city and state, or a ZIP code. Then run:

```sh
cargo run --bin ingest-rentcast
```

`RENTCAST_MAX_PAGES` defaults to `1` so development runs do not unexpectedly consume a large API allowance. Each request uses RentCast's maximum page size of 500 listings to make efficient use of the monthly request quota. Do not run the ingestion command casually: every execution makes at least one billable API request.

LandEX records every RentCast attempt before sending it and enforces a hard ceiling of 45 attempts in any rolling 32-day window. This intentionally stays below the provider's 50-request monthly allowance. The guard must never be bypassed; requests made outside LandEX are not visible to it and should be avoided.

### Market aggregation

Generate or refresh market observations from normalized active listings with:

```sh
cargo run --bin refresh-markets
```

The aggregation produces median sale price, median monthly rent, gross rental yield, active inventory, and average days on market without depending on any one external provider.

## Verification

Run the fast backend suite with `cargo test`. PostgreSQL-backed ingestion tests are enabled with `cargo test --features integration-tests` and run automatically in continuous integration.
