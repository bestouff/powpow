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

### Atelier WhatsApp groups

Admins can set an optional **Groupe WhatsApp** invitation link for each atelier
under **Administration → Paramètres**. In WhatsApp, copy the group's **Invite via
link** URL (`https://chat.whatsapp.com/…`). Clear the field to remove the link.

A volunteer's profile, accessible from **Staff**, shows the groups for their
selected ateliers, including pending registrations. The links are visible to the
volunteer, members sharing their email, and admins. Joining or leaving an atelier
updates this list; joining the WhatsApp group itself requires following its link.
Several ateliers can share the same link. The profile shows each group once,
labelled with the person's corresponding ateliers.

### Calendar and planning

- Define **opening days** for the station
- Set **staffing needs** per atelier per day (number of volunteers needed)
- Volunteers mark their **presence** for specific days
- **Infos du jour** (`/today`) shows the day's weather forecast, current piste and téléski statuses, and volunteers by atelier and shift, using the server's local date. Access requires staff authentication and current membership. The page refreshes automatically every five minutes while visible, and catches up when returning to an overdue background tab. The Open-Meteo forecast for Saint-Hilaire at 1,200 m refreshes in the background every 15 minutes, independently of the dicton feature; an unavailable forecast does not block the planning.
- The system computes **deficits** (days where not enough volunteers have signed up)
- A **weekly Monday morning email** is sent to admins summarizing unimported memberships and upcoming staffing shortfalls

#### Personal calendar subscriptions

On your profile, open **Abonnement au calendrier** and create a subscription link.
Use **S'abonner au calendrier** with Apple Calendar or another compatible app,
or copy the URL into Google Calendar's **From URL** option on the Web, then enable
that calendar on Android. Members sharing an email can also manage each other's
subscription links.

The feed contains past and upcoming registered presences, with one all-day event
per atelier and date. Titles indicate the selected periods, such as
“Buvette — matin et après-midi” or “Damage — soirée et nuit”. Night shifts use
their planning date. Attendance changes update the same event; unregistering
removes it on the next refresh. Changes are made in PowPow, and calendar providers
choose their refresh interval, which can be several hours.

Each URL is private, read-only, and remains valid across logouts and restarts.
**Régénérer le lien** disables the previous URL; update your calendar subscriptions
after regenerating it. Subscription links are preserved in database backups.
Links use the configured `PUBLIC_BASE_URL`, including its deployment path prefix.

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

### Météo-France observation archive

PowPow polls Météo-France's [observation API](https://confluence-meteofrance.atlassian.net/wiki/spaces/OpenDataMeteoFrance/pages/853639294/API+Cibl+e+Donn+es+d+Observation)
at startup and every **six minutes**, storing observations in the PostgreSQL
`meteo` table. Configure `.env` for local development or `/etc/powpow.conf` for
the Debian service:

```dotenv
METEO_FRANCE_STATION_ID=38395004
METEO_FRANCE_APPLICATION_ID=your_application_credential
```

Station **38395004 — SAINT-HILAIRE_SAPC** is at **45.315000, 5.884833, 990 m**,
according to Météo-France's station catalogue. The nearby **ST HILAIRE-NIVOSE**
station (38395403) is at 1,756 m.

To obtain the application credential, sign in to [the Météo-France API portal](https://portail-api.meteofrance.fr),
open the account menu → **Mes API**, select the observation API, then choose
**Générer Token**. Copy the value after `Authorization: Basic` in the token
generation cURL command into `METEO_FRANCE_APPLICATION_ID`. PowPow exchanges it
for access tokens using OAuth2 client credentials and renews them automatically.
This credential takes precedence over `METEO_FRANCE_TOKEN`, which can instead
hold a temporary bearer token. A standalone bearer token must be replaced,
followed by an application restart, when it expires. Leaving both credentials
empty disables collection.

The importer calls `/public/DPObs/v2/station/infrahoraire-6m` for the latest
observation. Each row is keyed by station and sensor measurement time
(`observed_at`, from `validity_time`), so repeated polls do not create duplicates.
Newer provider corrections update existing rows. Temperatures are stored in °C,
rainfall over six minutes in mm, snow depth in m, wind speeds in m/s, directions
in degrees, humidity in percent, and pressures in Pa. Unavailable measurements
remain `NULL`; `raw_data` preserves the complete observation in the provider's
original units, including fields without a dedicated column. All timestamps use
`TIMESTAMPTZ`. The `meteo` table is included in database backups and restores.

Failed requests are logged and retried next cycle. Collection archives future
observations while PowPow runs; it does not backfill downtime. To apply migrations
and import once without starting the web server, run `powpow --sync-meteo`.

### Webcam

The public `/webcam` page displays the latest camera image, the configured title,
the logo configured in the `hero-subtitle` content block, and available
Météo-France measurements as an HTML/CSS overlay. Weather includes its observation
time; missing measurements are omitted.
Wind is displayed in km/h, snow depth in cm, and pressure in hPa. Rainfall in mm
is accumulated over the hour ending at the displayed observation time, summing
ten six-minute readings. The total is omitted if any of these readings is missing.
Visitors can choose a local date and scrub through its images with a slider.
Dragging displays a small JPEG preview and its local timestamp; releasing loads
the full image and its matching weather. Four shortcuts select the current image
or the capture nearest to one, three, or five hours ago, before or after the target
time. Equally close captures favor the earlier image. A shortcut is disabled only
when no capture is available within 30 minutes before or after its target time.
Selecting the latest capture returns to the live view with automatic updates. Use
`/webcam?camera=tkecole` to select a specific camera. The latest
view checks for image and weather updates every minute. Archive views stay on the
selected image and show the most recent reading at or before its reception time,
within 30 minutes; older or missing readings are omitted.
Displayed image and weather times, along with archive dates, use the server's
local timezone (`TZ`, set to `Europe/Paris` in Docker Compose), including daylight
saving time changes.
The original image files retain their original pixels.

`/webcam/tkecole.jpg` serves that camera's latest image as a JPEG. Other cameras
use their source name in the same URL. Responses have `ETag`, `Last-Modified`,
and `Cache-Control: public, max-age=60, must-revalidate`: browsers reuse the image
for one minute, then receive `304 Not Modified` if it has not changed.

PowPow generates missing previews at startup and creates them with each new
capture. Thumbnails fit within 480 × 270 pixels and use the original filename
plus `_small.jpg` (for example, `2026-11-12T12:00:00.000000000Z_tkecole.jpg_small.jpg`).
Each local day and camera also has an atomically updated
`2026-11-12_tkecole_list.json` file, served at `/webcam/2026-11-12_tkecole_list.json`.
Its `images` array lists original filenames, thumbnail filenames, reception
timestamps with their local offset, and display labels in chronological order.
Originals and thumbnails under `/webcam/images/` use immutable caching.
PNG/WebP originals also get a full-size JPEG for the stable `.jpg` URL.

#### Retrieve snapshots from a URL

Set the URL and period in `/etc/powpow.conf` (or `.env` for local development).
You can also choose a source name and display title:

```dotenv
DIRECT_WEBCAM_URL=https://camera.example/snapshot.jpg
DIRECT_WEBCAM_PERIOD=6m
DIRECT_WEBCAM_NAME=pistes
DIRECT_WEBCAM_TITLE="Vue sur les pistes"
```

The period is a positive whole number followed by `s` (seconds), `m` (minutes),
`h` (hours), or `d` (days), for example `30s`, `6m`, or `1h`. Leaving both
settings empty disables retrieval; incomplete or invalid settings fail startup.
Restart PowPow after changing these settings.

`DIRECT_WEBCAM_NAME` identifies the source in archived filenames and defaults to
`direct`. It accepts 1–64 ASCII letters, digits, hyphens, or underscores.
`DIRECT_WEBCAM_TITLE` labels this source's images on `/webcam`, including the page
heading and image overlay. It defaults to `FTPS_TITLE`, or `Webcam` when neither
title is set. When both sources run, the selected image's source name determines
which title appears.

For cameras with invalid or self-signed certificates, set
`DIRECT_WEBCAM_INSECURE_TLS=true` to skip TLS certificate and hostname verification
for direct snapshots, including redirects. This option defaults to `false` and
applies only to the direct webcam HTTP client.

PowPow fetches an image immediately at startup and then at the configured period.
It follows up to five HTTP redirects and requests a fresh response each time.
The source can be a public URL or a reachable camera on a private network.
The image format is detected from the downloaded bytes, so URLs without image
extensions also work. Each snapshot passes through the same decoder, size limits,
and atomic archive storage as FTPS uploads. Files use the configured source name,
for example `2026-11-04T12:00:00.000000000Z_pistes.jpg`, and appear on `/webcam`.
Each successful fetch creates an archive entry, even if the image is unchanged.

Requests time out after 30 seconds. Failed downloads and invalid images are
logged and retried next cycle; the last valid image stays visible. Retrievals run
one at a time and skip missed timer ticks. Direct retrieval and FTPS can run
together or independently, using `WEBCAM_DIRECTORY` for storage. Docker Compose
persists this directory in the `aghil_webcam_data` volume.

#### Receive snapshots over FTPS

PowPow embeds [ThirtyFTP](https://crates.io/crates/thirtyftp) for explicit FTPS
uploads. Configure one camera account in `/etc/powpow.conf`:

```dotenv
FTPS_TITLE="Vue sur les pistes"
FTPS_USERNAME=station
FTPS_PASSWORD=replace-with-a-random-password
FTPS_TLS_CERT=/etc/powpow-tls/fullchain.pem
FTPS_TLS_KEY=/etc/powpow-tls/privkey.pem
FTPS_LISTEN_ADDRESS=0.0.0.0:2121
FTPS_PASSIVE_PORTS=50000-50009
WEBCAM_DIRECTORY=/var/lib/powpow/webcam
# Set this only if the address seen by the server differs from its public address:
# FTPS_PASSIVE_HOST=webcam.example.com
```

`FTPS_TITLE` defaults to `Webcam`. Leaving both credentials empty disables the
upload listener; existing archives remain accessible. Incomplete credentials or
TLS settings fail startup with a configuration error. Both control and data
connections require TLS; active FTP and SFTP are unsupported.

The certificate must cover the hostname configured on the camera. Give the
`powpow` service read access to the certificate chain and private key, for example
by installing copies in `/etc/powpow-tls` with owner `root:powpow`, directory mode
`0750`, and file mode `0640`. When renewing the certificate, update those copies
and restart `powpow` to reload them.

For the defaults above, allow TCP port `2121` and TCP ports `50000:50009` through
the VPS firewall (including any provider firewall), then restart the service:

```sh
sudo ufw allow 2121/tcp
sudo ufw allow 50000:50009/tcp
sudo systemctl restart powpow
```

The Debian service creates `/var/lib/powpow` as its writable state directory.
For a custom directory outside that tree, also grant the service access with a
systemd `ReadWritePaths=` override and filesystem permissions for `powpow`.
Ports above 1023 work with the service's unprivileged account.

On the Reolink camera, select **FTPS Only**, port **2121**, passive transport,
file type **Picture**, and a **Timer** schedule with the desired interval.
Use **No overwrite**; PowPow assigns every upload a new archive filename.
Camera-created subdirectories are accepted as virtual folders and flattened.
Each completed upload becomes, for example:

```text
2026-11-02T14:30:00.123456789Z_station.jpg
```

The timestamp is the VPS reception time in UTC, with nanoseconds to distinguish
uploads. Supported image types are JPEG (`.jpg` or `.jpeg`), PNG, and WebP,
up to 20 MiB and 8192 × 8192 pixels. Images are decoded before publication;
failed transfers and invalid images do not replace the latest image. Files are
published atomically, and uploads cannot retrieve or delete archive files.

Archives survive restarts and are retained without automatic pruning. Include
`WEBCAM_DIRECTORY` in filesystem backups: the application's PostgreSQL backup
does not contain these images. After restoring or externally modifying the
archive directory, restart PowPow to rebuild its in-memory index.

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

SQL backups and restores are accessible via the admin panel or an automation token.
Exports use one consistent PostgreSQL snapshot and include import-review decisions,
news, pending newsletter cleanup, and sequence counters. Login sessions and pending
account tokens are excluded; restoring a backup clears them.

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

- Rust 1.96+ (edition 2024)
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
| `PUBLIC_BASE_URL`             | Public URL including any path prefix (e.g. `https://station.example/powpow`); defaults to `http://localhost:3000` |
| `HUGGINGFACE_TOKEN`           | Optional HF Inference API token (enables dicton du jour) |
| `RSS_NEWS_FEED`               | Optional RSS feed URL (enables news section)             |
| `FTPS_TITLE`                 | Public webcam title (defaults to `Webcam`)               |
| `FTPS_USERNAME` / `FTPS_PASSWORD` | Optional webcam upload account (enables embedded FTPS) |
| `FTPS_TLS_CERT` / `FTPS_TLS_KEY` | PEM certificate chain and private key for FTPS          |
| `DIRECT_WEBCAM_URL` / `DIRECT_WEBCAM_PERIOD` | Optional snapshot URL and interval with units (e.g., `6m`); set both |
| `DIRECT_WEBCAM_NAME`          | Direct snapshot source name in archive filenames (defaults to `direct`) |
| `DIRECT_WEBCAM_TITLE`         | Display title for direct snapshots (defaults to `FTPS_TITLE`, or `Webcam`) |
| `DIRECT_WEBCAM_INSECURE_TLS`  | Skip TLS certificate verification for direct snapshots (`true` or `false`; defaults to `false`) |
| `WEBCAM_DIRECTORY`           | Image archive directory (defaults to `/var/lib/powpow/webcam`) |
| `SYNC_TOKEN` / `BACKUP_TOKEN` | Optional tokens for headless API access                  |

See `.env.example` for the full list including SMTP/Gmail and Mailchimp settings.

For a Debian/systemd installation, set `PUBLIC_BASE_URL` in `/etc/powpow.conf`
to the address visitors use, then restart `powpow`. Emailed authentication links
use this address rather than request headers.

### Run with Docker for development

Set `DEV_POSTGRES_PASSWORD` in `.env` to a random hex password (for example, from
`openssl rand -hex 24`). For an existing database volume, use its current password.

```sh
docker compose up --build
```

This starts PostgreSQL 17 and the application on loopback ports 5432 and 3000.
Migrations run automatically on startup. Production uses the Debian package below.

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

The application runs database migrations on startup via SQLx.

## Code quality

The project enforces strict linting:

```sh
cargo clippy --locked --all-targets -- -Dwarnings -Dclippy::pedantic
cargo fmt
cargo test --locked
```

PostgreSQL and HTTP regression tests are ignored by default. Run them against a
disposable local PostgreSQL database with a role allowed to create test databases:

```sh
DATABASE_URL=postgres://user:password@127.0.0.1/powpow_test \
  cargo test --locked -- --include-ignored
```

These tests cover account authorization, private profile data, malformed dates,
mail errors and limits, concurrent planning, role removal, and backup round trips.
`Cargo.lock` is tracked so Debian builds use the tested dependency versions.

## Project structure

```
src/
  main.rs          # App setup, routes, state, background tasks
  auth.rs          # Passwordless auth extractors (RequireStaff/Chief/Admin/God)
  config.rs        # Environment/config parsing
  database.rs      # SQL queries and backup/restore
  database/        # Transactional imports, account changes, planning, and DB tests
  dicton.rs        # Daily AI-generated "dicton du jour" (weather + HF API)
  helloasso.rs     # HelloAsso API client
  mailchimp.rs     # Mailchimp API client
  models.rs        # Data models (Staff, Atelier, Equipment, NewsRow, etc.)
  news.rs          # RSS feed sync, image download, XML entity decoding
  personal_email.rs # Shared typed mail delivery
  public_http.rs   # Public-only, size-limited RSS downloads
  qualifications.rs # Qualification date validation and checked expiry
  rate_limit.rs    # Bounded mail request limits
  http_tests.rs    # Regression tests against the assembled router
  routes/          # Axum route handlers
  templates/       # Maud HTML templates
static/
  powpow.css       # Styles (embedded at compile time)
  powpow.js        # Client-side JS (embedded at compile time)
migrations/        # SQLx PostgreSQL migrations
```
