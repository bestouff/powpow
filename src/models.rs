use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct Photo {
    pub id: uuid::Uuid,
    pub photo_data: Vec<u8>,
    pub mime_type: String,
    pub photographer_id: uuid::Uuid,
    pub is_frontpage: bool,
    pub is_staff: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FromRow<'_, sqlx::postgres::PgRow> for Photo {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Photo {
            id: row.try_get("id")?,
            photo_data: row.try_get("photo_data")?,
            mime_type: row.try_get("mime_type")?,
            photographer_id: row.try_get("photographer_id")?,
            is_frontpage: row.try_get("is_frontpage")?,
            is_staff: row.try_get("is_staff")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

/// Photo metadata without the binary blob — used for listings.
#[derive(Debug, Clone, Serialize)]
pub struct PhotoMeta {
    pub id: uuid::Uuid,
    pub mime_type: String,
    pub photographer_id: uuid::Uuid,
    pub is_frontpage: bool,
    pub is_staff: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FromRow<'_, sqlx::postgres::PgRow> for PhotoMeta {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(PhotoMeta {
            id: row.try_get("id")?,
            mime_type: row.try_get("mime_type")?,
            photographer_id: row.try_get("photographer_id")?,
            is_frontpage: row.try_get("is_frontpage")?,
            is_staff: row.try_get("is_staff")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_type")]
pub struct User {
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub zip_code: Option<String>,
    pub country: Option<String>,
    pub birth_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_sync_at: Option<DateTime<Utc>>,
}

impl FromRow<'_, sqlx::postgres::PgRow> for User {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(User {
            email: row.try_get("email")?,
            first_name: row.try_get("first_name")?,
            last_name: row.try_get("last_name")?,
            phone: row.try_get("phone")?,
            address: row.try_get("address")?,
            city: row.try_get("city")?,
            zip_code: row.try_get("zip_code")?,
            country: row.try_get("country")?,
            birth_date: row.try_get("birth_date")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            last_sync_at: row.try_get("last_sync_at")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "membership_type")]
pub struct Membership {
    pub helloasso_order_id: i64,
    pub helloasso_item_id: i64,
    pub payer_email: Option<String>,

    pub beneficiary_first_name: Option<String>,
    pub beneficiary_last_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,

    pub item_name: Option<String>,
    pub item_type: Option<String>,
    pub tier_name: Option<String>,
    pub amount: Option<i32>,
    pub order_date: Option<DateTime<Utc>>,
    pub comment: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FromRow<'_, sqlx::postgres::PgRow> for Membership {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Membership {
            helloasso_order_id: row.try_get("helloasso_order_id")?,
            helloasso_item_id: row.try_get("helloasso_item_id")?,
            payer_email: row.try_get("payer_email")?,
            beneficiary_first_name: row.try_get("beneficiary_first_name")?,
            beneficiary_last_name: row.try_get("beneficiary_last_name")?,
            phone: row.try_get("phone")?,
            email: row.try_get("email")?,
            item_name: row.try_get("item_name")?,
            item_type: row.try_get("item_type")?,
            tier_name: row.try_get("tier_name")?,
            amount: row.try_get("amount")?,
            order_date: row.try_get("order_date")?,
            comment: row.try_get("comment")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

// Membership with staff import status
#[derive(Debug, Clone, Serialize)]
pub struct MembershipWithStatus {
    pub membership: Membership,
    pub season: i16,
    pub has_staff: bool,
    pub is_double_subscription: bool,
}

// Staff model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Staff {
    pub id: uuid::Uuid,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub email: String,
    pub comment: String,
    pub is_admin: bool,
    pub is_god: bool,
    pub token: Option<uuid::Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FromRow<'_, sqlx::postgres::PgRow> for Staff {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Staff {
            id: row.try_get("id")?,
            first_name: row.try_get("first_name")?,
            last_name: row.try_get("last_name")?,
            phone: row.try_get("phone")?,
            email: row.try_get("email")?,
            comment: row.try_get("comment")?,
            is_admin: row.try_get("is_admin")?,
            is_god: row.try_get("is_god")?,
            token: row.try_get("token")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

// Staff with latest paid season
#[derive(Debug, Clone, Serialize)]
pub struct StaffWithSeason {
    pub staff: Staff,
    pub latest_season: Option<i16>,
    pub match_type: StaffMatchType,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum StaffMatchType {
    ExactBoth,          // Both email and name match exactly (highest priority)
    DoubleSubscription, // Exact name match but already paid for this season (likely double subscription)
    ExactName,          // Name matches exactly (different email, no payment yet)
    ExactEmail,         // Beneficiary email matches exactly but name differs
    PayerEmailMatch,    // Payer email matches (but beneficiary email differs) - lower priority
    SimilarEmail,       // Fuzzy email match
    SimilarName,        // Fuzzy name match (lowest priority)
}

// Atelier model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Atelier {
    pub id: uuid::Uuid,
    pub name: String,
    pub slug: String,
    pub needs_validation: bool,
    pub default_nightly: bool,
    pub icon: String,
    pub opening_day_typical_needed: i16,
}

impl FromRow<'_, sqlx::postgres::PgRow> for Atelier {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Atelier {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            slug: row.try_get("slug")?,
            needs_validation: row.try_get("needs_validation")?,
            default_nightly: row.try_get("default_nightly")?,
            icon: row.try_get("icon")?,
            opening_day_typical_needed: row.try_get("opening_day_typical_needed")?,
        })
    }
}

// Role model (staff assignment to an atelier)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub staff: uuid::Uuid,
    pub atelier: uuid::Uuid,
    pub validated: bool,
    pub chief: bool,
}

impl FromRow<'_, sqlx::postgres::PgRow> for Role {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Role {
            staff: row.try_get("staff")?,
            atelier: row.try_get("atelier")?,
            validated: row.try_get("validated")?,
            chief: row.try_get("chief")?,
        })
    }
}

// Qualification model (type of training/certification)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Qualification {
    pub id: i32,
    pub name: String,
    pub duration: Option<i16>, // years valid, None = lifelong
}

impl FromRow<'_, sqlx::postgres::PgRow> for Qualification {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Qualification {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            duration: row.try_get("duration")?,
        })
    }
}

// Staff qualification record (staff member obtained a qualification)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffQualif {
    pub id: i32,
    pub staff: uuid::Uuid,
    pub qualification: i32,
    pub obtained_date: chrono::NaiveDate,
}

impl FromRow<'_, sqlx::postgres::PgRow> for StaffQualif {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(StaffQualif {
            id: row.try_get("id")?,
            staff: row.try_get("staff")?,
            qualification: row.try_get("qualification")?,
            obtained_date: row.try_get("obtained_date")?,
        })
    }
}

// Need model (day when staff are needed for an atelier)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Need {
    pub id: uuid::Uuid,
    pub day: chrono::NaiveDate,
    pub atelier: uuid::Uuid,
    pub quantity: i16,
    pub nightly: bool,
}

impl FromRow<'_, sqlx::postgres::PgRow> for Need {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Need {
            id: row.try_get("id")?,
            day: row.try_get("day")?,
            atelier: row.try_get("atelier")?,
            quantity: row.try_get("quantity")?,
            nightly: row.try_get("nightly")?,
        })
    }
}

// Opening day status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "opening_day_status", rename_all = "lowercase")]
pub enum OpeningDayStatus {
    Reserved,
    Validated,
    Canceled,
}

impl std::fmt::Display for OpeningDayStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reserved => write!(f, "reserved"),
            Self::Validated => write!(f, "validated"),
            Self::Canceled => write!(f, "canceled"),
        }
    }
}

// Opening day model (days when the ski station is open)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpeningDay {
    pub day: chrono::NaiveDate,
    pub status: OpeningDayStatus,
}

impl FromRow<'_, sqlx::postgres::PgRow> for OpeningDay {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(OpeningDay {
            day: row.try_get("day")?,
            status: row.try_get("status")?,
        })
    }
}

// Cash/check payment model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cash {
    pub id: uuid::Uuid,
    pub first_name: String,
    pub last_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub date: chrono::NaiveDate,
    pub amount: i32,
    pub is_membership: bool,
    pub payment_method: String,
}

impl FromRow<'_, sqlx::postgres::PgRow> for Cash {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Cash {
            id: row.try_get("id")?,
            first_name: row.try_get("first_name")?,
            last_name: row.try_get("last_name")?,
            email: row.try_get("email")?,
            phone: row.try_get("phone")?,
            date: row.try_get("date")?,
            amount: row.try_get("amount")?,
            is_membership: row.try_get("is_membership")?,
            payment_method: row.try_get("payment_method")?,
        })
    }
}

// Payment history (unified view of HelloAsso + cash payments for a staff member)
#[derive(Debug, Clone, Serialize)]
pub struct PaymentHistoryEntry {
    pub season: i16,
    pub source: String,       // "helloasso", "cash", "check"
    pub date: Option<String>, // formatted DD/MM/YYYY
    pub amount: Option<i32>,  // in euros
    pub item_type: String,    // "Don", "Adhésion"
    pub first_name: String,
    pub last_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub payer_email: Option<String>,
}

// HelloAsso API Models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAssoTokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub expires_in: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAssoUserResponse {
    pub data: Vec<HelloAssoUser>,
    pub pagination: HelloAssoPagination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAssoEventResponse {
    pub data: Vec<HelloAssoEvent>,
    pub pagination: HelloAssoPagination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAssoEvent {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAssoOrder {
    pub id: i64,
    pub date: DateTime<Utc>,
    pub amount: HelloAssoAmount,
    pub payer: HelloAssoUser,
    pub items: Vec<HelloAssoOrderItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAssoOrderItem {
    pub id: i64,
    pub name: Option<String>,
    pub state: String,
    pub amount: i64,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(rename = "priceCategory")]
    pub price_category: Option<String>,
    #[serde(rename = "tierId")]
    pub tier_id: Option<i64>,
    #[serde(rename = "tierDescription")]
    pub tier_description: Option<String>,
    pub user: Option<HelloAssoUser>,
    #[serde(rename = "customFields", default)]
    pub custom_fields: Vec<HelloAssoCustomField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAssoCustomField {
    pub id: i64,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub type_: String,
    pub answer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAssoUser {
    pub id: Option<String>,
    #[serde(rename = "firstName")]
    pub first_name: Option<String>,
    #[serde(rename = "lastName")]
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    #[serde(rename = "zipCode")]
    pub zip_code: Option<String>,
    pub country: Option<String>,
    #[serde(rename = "birthDate")]
    pub birth_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAssoAmount {
    pub total: i64,
    pub vat: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAssoOrdersResponse {
    pub data: Vec<HelloAssoOrder>,
    pub pagination: HelloAssoPagination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAssoPagination {
    #[serde(rename = "pageIndex")]
    pub page_index: i32,
    #[serde(rename = "pageSize")]
    pub page_size: i32,
    #[serde(rename = "totalCount")]
    pub total_count: i32,
    #[serde(rename = "totalPages")]
    pub total_pages: i32,
    #[serde(rename = "continuationToken")]
    pub continuation_token: Option<String>,
}

// Equipment type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "equipment_type", rename_all = "kebab-case")]
pub enum EquipmentType {
    SkiSlope,
    SkiTow,
}

impl std::fmt::Display for EquipmentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SkiSlope => write!(f, "ski-slope"),
            Self::SkiTow => write!(f, "ski-tow"),
        }
    }
}

// Piste difficulty enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "piste_difficulty", rename_all = "lowercase")]
pub enum PisteDifficulty {
    Verte,
    Bleue,
    Rouge,
    Noire,
}

impl PisteDifficulty {
    /// CSS color for this difficulty level.
    #[must_use]
    pub const fn css_color(self) -> &'static str {
        match self {
            Self::Verte => "#4caf50",
            Self::Bleue => "#2196f3",
            Self::Rouge => "#f44336",
            Self::Noire => "#212121",
        }
    }
}

// Equipment status enum (3-state: open / closed / partial)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "equipment_status", rename_all = "lowercase")]
pub enum EquipmentStatus {
    Open,
    Closed,
    Partial,
}

impl EquipmentStatus {
    /// CSS class suffix for progress bars.
    #[must_use]
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::Open => "is-open",
            Self::Closed => "is-closed",
            Self::Partial => "is-partial",
        }
    }

    /// French label for the status.
    #[must_use]
    pub const fn label_piste(self) -> &'static str {
        match self {
            Self::Open => "Ouverte",
            Self::Closed => "Fermée",
            Self::Partial => "Partielle",
        }
    }

    /// French label for téléskis (masculine).
    #[must_use]
    pub const fn label_tow(self) -> &'static str {
        match self {
            Self::Open => "Ouvert",
            Self::Closed => "Fermé",
            Self::Partial => "Partiel",
        }
    }

    /// Cycle to the next status: closed → partial → open → closed.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Closed => Self::Partial,
            Self::Partial => Self::Open,
            Self::Open => Self::Closed,
        }
    }
}

impl std::fmt::Display for EquipmentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Closed => write!(f, "closed"),
            Self::Partial => write!(f, "partial"),
        }
    }
}

// Equipment model
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct Equipment {
    pub id: uuid::Uuid,
    pub name: String,
    pub equipment_type: EquipmentType,
    pub status: EquipmentStatus,
    pub difficulty: Option<PisteDifficulty>,
}

impl FromRow<'_, sqlx::postgres::PgRow> for Equipment {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Equipment {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            equipment_type: row.try_get("equipment_type")?,
            status: row.try_get("status")?,
            difficulty: row.try_get("difficulty")?,
        })
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAssoErrorResponse {
    pub error: String,
    pub error_description: Option<String>,
}

// ── CMS content blocks ──────────────────────────────────────────────

/// An editable content block for the frontpage CMS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlock {
    pub slug: String,
    pub title: String,
    pub body: String,
    pub image_id: Option<uuid::Uuid>,
    pub link_url: Option<String>,
    pub link_label: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl FromRow<'_, sqlx::postgres::PgRow> for ContentBlock {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(ContentBlock {
            slug: row.try_get("slug")?,
            title: row.try_get("title")?,
            body: row.try_get("body")?,
            image_id: row.try_get("image_id")?,
            link_url: row.try_get("link_url")?,
            link_label: row.try_get("link_label")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl ContentBlock {
    /// Render the markdown body to sanitised HTML.
    ///
    /// Uses `pulldown-cmark` for Markdown→HTML then `ammonia` to strip
    /// dangerous tags (`<script>`, `<iframe>`, event-handler attributes, …)
    /// while keeping safe formatting elements.
    #[must_use]
    pub fn render_body(&self) -> String {
        use pulldown_cmark::{Options, Parser, html};
        let parser = Parser::new_ext(&self.body, Options::all());
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);
        ammonia::clean(&html_output)
    }
}

/// Thin wrapper around a `HashMap<String, ContentBlock>` that always returns
/// a reference: the real block when present, or a built-in placeholder whose
/// title and body make it obvious the content is missing.
#[derive(Clone)]
pub struct ContentMap {
    blocks: std::collections::HashMap<String, ContentBlock>,
    placeholder: ContentBlock,
}

impl ContentMap {
    /// Build from the map returned by `get_all_contents()`.
    #[must_use]
    pub fn new(blocks: std::collections::HashMap<String, ContentBlock>) -> Self {
        Self {
            blocks,
            placeholder: ContentBlock {
                slug: String::new(),
                title: "[contenu manquant]".to_string(),
                body: "[contenu manquant]".to_string(),
                image_id: None,
                link_url: None,
                link_label: None,
                updated_at: Utc::now(),
            },
        }
    }

    /// Get a content block by slug, or the placeholder if missing.
    #[must_use]
    pub fn get(&self, slug: &str) -> &ContentBlock {
        self.blocks.get(slug).unwrap_or(&self.placeholder)
    }
}

/// A CMS image stored in the database (separate from volunteer photos).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ContentImage {
    pub id: uuid::Uuid,
    pub data: Vec<u8>,
    pub content_type: String,
    pub filename: String,
    pub created_at: DateTime<Utc>,
}

impl FromRow<'_, sqlx::postgres::PgRow> for ContentImage {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(ContentImage {
            id: row.try_get("id")?,
            data: row.try_get("data")?,
            content_type: row.try_get("content_type")?,
            filename: row.try_get("filename")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

// ── News (RSS feed items stored in DB) ──────────────────────────────

/// A news item row for display (without the image binary data).
#[derive(Clone, Debug)]
pub struct NewsRow {
    pub id: uuid::Uuid,
    pub text: String,
    pub link: String,
    pub pub_date: Option<DateTime<Utc>>,
    pub has_image: bool,
}
