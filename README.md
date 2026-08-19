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

The API verifies its database connection and applies pending migrations before it starts accepting requests.

The initial service endpoints are:

- `GET /api/v1/health` — confirms the API process is running.
- `GET /api/v1/ready` — confirms the API can reach PostgreSQL.
