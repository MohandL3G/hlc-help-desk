# IT Desk — Rust starter

A small, Arabic RTL IT-request tracker with a visual style inspired by the `archive-hlc` app (grey canvas, red accent, rounded panels, right-side navigation, responsive tables and a light/dark theme).

## Included in this starter

- Dashboard counts and recent requests.
- Create a ticket and assign an automatic ID such as `IT-000012`.
- List/search/filter tickets.
- Update status and add an explanation of the action taken.
- A chronological `ticket_events` history. The UI has no delete button.
- Print-friendly service report with requester/technician signature lines.
- SQLite persistence and Docker Compose.
- Mobile layout and light/dark theme.

## Important: prototype only

**Do not expose this build to employees or the public yet.** It intentionally has no authentication, user accounts, CSRF protection, attachment upload, email notifications or role-based permissions. The current actor in the history is a fixed label, not a verified user. The event history is useful for prototyping, but it is not tamper-proof against someone with database/server access. Add authentication and permissions before real company use, then add evidence uploads, independent backups and restore tests.

## Run locally with Rust

Install Rust using the official instructions: <https://www.rust-lang.org/tools/install>.

From this project directory:

```bash
cargo run
```

The database is created at `data/tickets.db` by default. Open <http://localhost:3000>.

To change the database path or port:

```bash
DB_PATH=data/dev.db PORT=3000 cargo run
```

## Run using Docker Compose

```bash
docker compose up --build -d
```

Open <http://localhost:3800>. The SQLite file is persisted in the `it-desk-data` volume. The host port is bound to `127.0.0.1` deliberately, so this does not publish the unauthenticated prototype to your network by default.

View logs:

```bash
docker compose logs -f it-desk
```

Stop:

```bash
docker compose down
```

The named data volume remains after `docker compose down`. Avoid `docker compose down -v` unless you intend to delete it.

## Suggested next milestones

1. Add login, secure sessions, user roles (requester, IT staff, administrator), CSRF protection and rate limits.
2. Add attachment upload with size/type limits and storage outside the container; log upload/download events.
3. Record the authenticated user ID for every change; do not let users silently edit or delete historical events.
4. Add backup/restore scripts that cover both the database and uploaded evidence.
5. Add user notifications, then consider WhatsApp integration only after the ticket flow is stable.
6. Confirm the company's signed-paper retention requirements before treating the generated printout as a replacement for an original form.

## Code layout

```text
src/main.rs                 Axum routes, SQLite queries and audit-event creation
templates/                  Askama server-rendered pages
static/style.css             responsive RTL styling
static/app.js                theme toggle and list filtering
Dockerfile                   multi-stage Rust build
docker-compose.yml           local-only host binding plus persistent volume
```
