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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FromRow<'_, sqlx::postgres::PgRow> for PhotoMeta {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(PhotoMeta {
            id: row.try_get("id")?,
            mime_type: row.try_get("mime_type")?,
            photographer_id: row.try_get("photographer_id")?,
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
