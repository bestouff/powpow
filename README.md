# PowPow

**Pistes, Organisation, Week-end, Planning, Optimisation, Wouah!**

PowPow is a web application for managing volunteers, memberships, and plannings for a ski station in the French Alps, in Chartreuse, in the beautiful village of Le Plateau des Petites Roches: https://station-ski-saint-hilaire.fr/. It integrates with [HelloAsso](https://helloasso.com/) for billing and with [Mailchimp](https://mailchimp.com/) for mailings to staff.

> NB: the primary source is on [Codeberg](https://codeberg.org/bestouff/powpow), if you are on GitHub you're just seeing a copy for visibility

## Features

### Passwordless authentication

No passwords to remember. Volunteers log in by searching their name, receiving an email with a magic link, and clicking it. The trusted tier is their email address.

### Staff and privilege levels

Five privilege tiers:

| Level         | Access                                                                                                |
| ------------- | ----------------------------------------------------------------------------------------------------- |
| **Anonymous** | See the frontpage, current opening status, equipments availability                                    |
| **Staff**     | View their own profile, register for ateliers, mark presence on the calendar                          |
| **Chief**     | Manage the ateliers they lead, validate role requests from volunteers                                 |
| **Admin**     | Full management: import memberships, manage staff, edit content                                       |
| **God**       | Everything above, plus the ability to grant/revoke admin and god status, backup /restore the database |

### Ateliers and roles

The station's work is organized into _ateliers_ (workshops/activity groups). Volunteers can register interest in ateliers, and atelier chiefs can approve or reject requests. Each atelier can optionally require validation before a volunteer is confirmed.

### Calendar and planning

- Define **opening days** for the station
- Set **staffing needs** per atelier per day (number of volunteers needed)
- Volunteers mark their **presence** for specific days
- The system computes **deficits** (days where not enough volunteers have signed up)
- A **weekly Monday morning email** is sent to admins summarizing unimported memberships and upcoming staffing shortfalls

### Ski slopes and tows

Real-time status management for ski slopes and ski tows, displayed on the public frontpage. Each piece of equipment has a three-state status (open / partial / closed) that admins can toggle with one click. Slopes also carry a difficulty level (green, blue, red, black).

### HelloAsso integration

- Periodic sync pulls all orders and user data from the HelloAsso API
- Webhook endpoint receives real-time notifications of new memberships
- Membership data is matched and imported into the local staff database
- Supports both HelloAsso online payments and manual cash/check payments

### Mailchimp integration

- Syncs the staff list to a Mailchimp audience (upserts members with first/last name)
- Can send campaign emails to all members or individual staff

### Photo management

- Upload photos with photographer attribution
- Select which photos appear in the frontpage hero carousel via a frontpage flag
- Admin interface for browsing, deleting, and toggling photos

### Dicton du jour

An AI-generated daily paragraph displayed on the homepage, inspired by the current weather at the station. Every day, the application fetches weather data from the [Open-Meteo](https://open-meteo.com/) API and sends a prompt to a language model (DeepSeek V3 Turbo via the [Hugging Face](https://huggingface.co/) Inference API) asking for a whimsical French "dicton" about the mountains. The result is cached for the day and rendered as styled HTML on the frontpage. The feature is opt-in: it activates only when a `HUGGINGFACE_TOKEN` is configured.

### News feed

The homepage displays the latest posts from the association's Facebook page, fetched via an RSS bridge feed. A background task syncs the feed every 15 minutes, downloading post text and images into PostgreSQL. Images are stored as BYTEA and served from the database, avoiding any external CDN dependency at render time. The feature is opt-in: it activates only when an `RSS_NEWS_FEED` URL is configured.
Examples of suitable RSS bridges: https://fetchrss.com or https://rss.app

### CMS content blocks

Editable content blocks stored in the database, rendered with Markdown (via pulldown-cmark). Used for the homepage sections, navbar links, footer content, and driving indications. Admins edit content through a built-in content editor.

### Qualifications

Track staff qualifications (e.g. certifications, training) with type, obtained date, and expiration. Displayed on individual staff profiles.

### Audit journal

Every significant action (membership imports, role changes, qualification updates, contact edits, etc.) is logged. The audit page resolves staff UUIDs to names for readability.

### Backup and restore

Full database backup (JSON export of all 17+ tables) and restore, accessible via the admin panel or via automation token for headless/cron usage.

## Tech stack

| Component        | Technology                                                              |
| ---------------- | ----------------------------------------------------------------------- |
| Language         | Rust 2024 edition                                                       |
| Web framework    | [Axum](https://github.com/tokio-rs/axum) 0.8                            |
| Async runtime    | [Tokio](https://tokio.rs/)                                              |
| Database         | PostgreSQL 15 via [SQLx](https://github.com/launchbadge/sqlx) 0.8       |
| HTML templating  | [Maud](https://maud.lambda.xyz/) 0.27                                   |
| CSS framework    | [Bulma](https://bulma.io/)                                              |
| Email            | [Lettre](https://github.com/lettre/lettre) (SMTP) or Gmail API          |
| Markdown         | [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) 0.13 |
| HTML sanitizer   | [ammonia](https://github.com/rust-ammonia/ammonia)                      |
| RSS parsing      | [rss](https://crates.io/crates/rss) 2.0                                 |
| Containerization | Docker with multi-stage build (cargo-chef for layer caching)            |

All CSS and JS are embedded in the binary via `include_str!()` and served from memory -- no external static file server required. The application is a single self-contained binary.

## Getting started

### Prerequisites

- Rust 1.93+ (edition 2024)
- PostgreSQL 15+
- A HelloAsso API account

### Configuration

Copy `.env.example` to `.env` and fill in the values:

```sh
cp .env.example .env
```

If you are using the debian package (you can build one via `make deb`) then instead edit the `/etc/powpow.conf` file.

Key variables:

| Variable                      | Description                                              |
| ----------------------------- | -------------------------------------------------------- |
| `DATABASE_URL`                | PostgreSQL connection string                             |
| `HELLOASSO_CLIENT_ID`         | HelloAsso API client ID                                  |
| `HELLOASSO_CLIENT_SECRET`     | HelloAsso API client secret                              |
| `HELLOASSO_ASSOCIATION_SLUG`  | Your association's slug on HelloAsso                     |
| `MAIL_METHOD`                 | `smtp` or `gmail`                                        |
| `COOKIE_SECRET`               | Random string (64+ hex chars) for signed session cookies |
| `HUGGINGFACE_TOKEN`           | Optional HF Inference API token (enables dicton du jour) |
| `RSS_NEWS_FEED`               | Optional RSS feed URL (enables news section)             |
| `SYNC_TOKEN` / `BACKUP_TOKEN` | Optional tokens for headless API access                  |

See `.env.example` for the full list including SMTP/Gmail and Mailchimp settings.

### Run with Docker (recommended)

```sh
docker compose up --build
```

This starts PostgreSQL 15 and the application on port 3000. Migrations run automatically on startup.
**This is for development only, if you want to use it to deploy change the password in the docker file !**

### Run as a Debian package (recommended for Debian servers)

```sh
dpkg -i powpow<version>.deb
```

... and that's all. Database migrations, updates and maintaining the server alive are handled via systemd.

To make your website available globally, redirect localhost:3000 to the world. Example with apache: create `/etc/apache2/sites-available/your_ski_station_name.conf`

```apache
<VirtualHost *:80>
    ServerName your_ski_station_name
    ServerAlias www.your_ski_station_name

    ProxyPreserveHost On
    ProxyPass / http://localhost:3000/
    ProxyPassReverse / http://localhost:3000/

    ErrorLog ${APACHE_LOG_DIR}/your_ski_station_name_error.log
    CustomLog ${APACHE_LOG_DIR}/your_ski_station_name_access.log combined
</VirtualHost>
```

Then enable it:

```sh
a2ensite your_ski_station_name.conf
```

Then make it available via SSL:

```sh
sudo certbot --apache -d your_ski_station_name -d your_ski_station_name
```

### Run locally

```sh
# Start PostgreSQL separately, then:
cargo build --release
./target/release/powpow
```

### Development

```sh
cargo run
```

The application runs database migrations on startup via SQLx. There are currently 30 migration files covering all schema changes.

## Code quality

The project enforces strict linting:

```sh
cargo clippy -- -Dclippy::pedantic   # must produce zero warnings
cargo fmt                              # always run after changes
```

## Project structure

```
src/
  main.rs          # App setup, routes, state, background tasks
  auth.rs          # Passwordless auth extractors (RequireStaff/Chief/Admin/God)
  config.rs        # Environment/config parsing
  database.rs      # All SQL queries (~3000 lines)
  dicton.rs        # Daily AI-generated "dicton du jour" (weather + HF API)
  helloasso.rs     # HelloAsso API client
  mailchimp.rs     # Mailchimp API client
  models.rs        # Data models (Staff, Atelier, Equipment, NewsRow, etc.)
  news.rs          # RSS feed sync, image download, XML entity decoding
  routes/          # Axum route handlers
  templates/       # Maud HTML templates
static/
  powpow.css       # Styles (embedded at compile time)
  powpow.js        # Client-side JS (embedded at compile time)
migrations/        # SQLx PostgreSQL migrations (001-031)
```
