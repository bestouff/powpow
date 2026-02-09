use crate::models::Staff;
use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct MailchimpClient {
    client: Client,
    api_key: String,
    server_prefix: String,
    list_id: String,
    from_name: String,
    from_email: String,
}

#[derive(Debug, Serialize)]
struct MemberBody {
    email_address: String,
    status_if_new: &'static str,
    merge_fields: MergeFields,
}

#[derive(Debug, Serialize)]
struct MergeFields {
    #[serde(rename = "FNAME")]
    fname: String,
    #[serde(rename = "LNAME")]
    lname: String,
}

#[derive(Debug, Serialize)]
struct CampaignCreateBody {
    #[serde(rename = "type")]
    type_: &'static str,
    recipients: CampaignRecipients,
    settings: CampaignSettings,
}

#[derive(Debug, Serialize)]
struct CampaignRecipients {
    list_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    segment_opts: Option<SegmentOpts>,
}

#[derive(Debug, Serialize)]
struct SegmentOpts {
    #[serde(rename = "match")]
    match_: &'static str,
    conditions: Vec<SegmentCondition>,
}

#[derive(Debug, Serialize)]
struct SegmentCondition {
    condition_type: &'static str,
    field: &'static str,
    op: &'static str,
    value: String,
}

#[derive(Debug, Serialize)]
struct CampaignSettings {
    subject_line: String,
    from_name: String,
    reply_to: String,
}

#[derive(Debug, Serialize)]
struct CampaignContentBody {
    html: String,
}

#[derive(Debug, Deserialize)]
struct CampaignResponse {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    title: Option<String>,
    detail: Option<String>,
}

impl MailchimpClient {
    pub fn new(
        api_key: String,
        server_prefix: String,
        list_id: String,
        from_name: String,
        from_email: String,
    ) -> Self {
        Self {
            client: Client::new(),
            api_key,
            server_prefix,
            list_id,
            from_name,
            from_email,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
            && !self.server_prefix.is_empty()
            && !self.list_id.is_empty()
            && !self.from_email.is_empty()
    }

    fn base_url(&self) -> String {
        format!("https://{}.api.mailchimp.com/3.0", self.server_prefix)
    }

    /// Add or update a member in the Mailchimp audience.
    /// Uses PUT with subscriber hash (MD5 of lowercase email) for upsert semantics.
    pub async fn upsert_member(
        &self,
        email: &str,
        first_name: &str,
        last_name: &str,
    ) -> Result<()> {
        let hash = md5_hex(email.trim().to_lowercase().as_bytes());
        let url = format!(
            "{}/lists/{}/members/{}",
            self.base_url(),
            self.list_id,
            hash
        );

        let body = MemberBody {
            email_address: email.trim().to_lowercase(),
            status_if_new: "subscribed",
            merge_fields: MergeFields {
                fname: first_name.to_string(),
                lname: last_name.to_string(),
            },
        };

        let response = self
            .client
            .put(&url)
            .basic_auth("aghil", Some(&self.api_key))
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let err: ErrorResponse = response.json().await.unwrap_or(ErrorResponse {
                title: Some("Unknown error".into()),
                detail: None,
            });
            return Err(anyhow!(
                "Mailchimp upsert_member failed ({}): {} – {}",
                status,
                err.title.unwrap_or_default(),
                err.detail.unwrap_or_default()
            ));
        }

        Ok(())
    }

    /// Sync all staff members to the Mailchimp audience.
    /// Returns (success_count, error_count).
    pub async fn sync_staff(&self, staff: &[Staff]) -> Result<(usize, usize)> {
        let mut ok = 0usize;
        let mut err_count = 0usize;

        for s in staff {
            match self.upsert_member(&s.email, &s.first_name, &s.last_name).await {
                Ok(()) => ok += 1,
                Err(e) => {
                    warn!("Mailchimp sync failed for {}: {}", s.email, e);
                    err_count += 1;
                }
            }
        }

        info!("Mailchimp sync done: {} ok, {} errors", ok, err_count);
        Ok((ok, err_count))
    }

    /// Send a campaign email to every member of the audience.
    pub async fn send_to_all(&self, subject: &str, html: &str) -> Result<String> {
        self.send_campaign(subject, html, None).await
    }

    /// Send a campaign email to a single member (identified by email).
    pub async fn send_to_member(&self, email: &str, subject: &str, html: &str) -> Result<String> {
        let segment = SegmentOpts {
            match_: "all",
            conditions: vec![SegmentCondition {
                condition_type: "EmailAddress",
                field: "EMAIL",
                op: "is",
                value: email.trim().to_lowercase(),
            }],
        };
        self.send_campaign(subject, html, Some(segment)).await
    }

    /// Create a campaign, set its content, and send it.
    /// Returns the campaign ID.
    async fn send_campaign(
        &self,
        subject: &str,
        html: &str,
        segment_opts: Option<SegmentOpts>,
    ) -> Result<String> {
        // 1. Create campaign
        let create_url = format!("{}/campaigns", self.base_url());

        let create_body = CampaignCreateBody {
            type_: "regular",
            recipients: CampaignRecipients {
                list_id: self.list_id.clone(),
                segment_opts,
            },
            settings: CampaignSettings {
                subject_line: subject.to_string(),
                from_name: self.from_name.clone(),
                reply_to: self.from_email.clone(),
            },
        };

        let response = self
            .client
            .post(&create_url)
            .basic_auth("aghil", Some(&self.api_key))
            .json(&create_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let err: ErrorResponse = response.json().await.unwrap_or(ErrorResponse {
                title: Some("Unknown error".into()),
                detail: None,
            });
            return Err(anyhow!(
                "Mailchimp create campaign failed ({}): {} – {}",
                status,
                err.title.unwrap_or_default(),
                err.detail.unwrap_or_default()
            ));
        }

        let campaign: CampaignResponse = response.json().await?;
        let campaign_id = campaign.id;

        info!("Created Mailchimp campaign {}", campaign_id);

        // 2. Set content
        let content_url = format!("{}/campaigns/{}/content", self.base_url(), campaign_id);

        let response = self
            .client
            .put(&content_url)
            .basic_auth("aghil", Some(&self.api_key))
            .json(&CampaignContentBody {
                html: html.to_string(),
            })
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let err: ErrorResponse = response.json().await.unwrap_or(ErrorResponse {
                title: Some("Unknown error".into()),
                detail: None,
            });
            return Err(anyhow!(
                "Mailchimp set content failed ({}): {} – {}",
                status,
                err.title.unwrap_or_default(),
                err.detail.unwrap_or_default()
            ));
        }

        // 3. Send
        let send_url = format!(
            "{}/campaigns/{}/actions/send",
            self.base_url(),
            campaign_id
        );

        let response = self
            .client
            .post(&send_url)
            .basic_auth("aghil", Some(&self.api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let err: ErrorResponse = response.json().await.unwrap_or(ErrorResponse {
                title: Some("Unknown error".into()),
                detail: None,
            });
            return Err(anyhow!(
                "Mailchimp send campaign failed ({}): {} – {}",
                status,
                err.title.unwrap_or_default(),
                err.detail.unwrap_or_default()
            ));
        }

        info!("Sent Mailchimp campaign {}", campaign_id);
        Ok(campaign_id)
    }
}

/// Compute the hex-encoded MD5 hash of `data`.
/// Mailchimp uses MD5(lowercase_email) as subscriber hash.
fn md5_hex(data: &[u8]) -> String {
    use std::fmt::Write;
    // Simple MD5 implementation inline to avoid adding a dependency.
    // We only need it for short email strings.
    let digest = md5_digest(data);
    let mut s = String::with_capacity(32);
    for b in &digest {
        write!(s, "{:02x}", b).unwrap();
    }
    s
}

// Minimal MD5 (RFC 1321) – only used for subscriber hashes.
fn md5_digest(input: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7,12,17,22, 7,12,17,22, 7,12,17,22, 7,12,17,22,
        5, 9,14,20, 5, 9,14,20, 5, 9,14,20, 5, 9,14,20,
        4,11,16,23, 4,11,16,23, 4,11,16,23, 4,11,16,23,
        6,10,15,21, 6,10,15,21, 6,10,15,21, 6,10,15,21,
    ];
    const K: [u32; 64] = [
        0xd76aa478,0xe8c7b756,0x242070db,0xc1bdceee,
        0xf57c0faf,0x4787c62a,0xa8304613,0xfd469501,
        0x698098d8,0x8b44f7af,0xffff5bb1,0x895cd7be,
        0x6b901122,0xfd987193,0xa679438e,0x49b40821,
        0xf61e2562,0xc040b340,0x265e5a51,0xe9b6c7aa,
        0xd62f105d,0x02441453,0xd8a1e681,0xe7d3fbc8,
        0x21e1cde6,0xc33707d6,0xf4d50d87,0x455a14ed,
        0xa9e3e905,0xfcefa3f8,0x676f02d9,0x8d2a4c8a,
        0xfffa3942,0x8771f681,0x6d9d6122,0xfde5380c,
        0xa4beea44,0x4bdecfa9,0xf6bb4b60,0xbebfbc70,
        0x289b7ec6,0xeaa127fa,0xd4ef3085,0x04881d05,
        0xd9d4d039,0xe6db99e5,0x1fa27cf8,0xc4ac5665,
        0xf4292244,0x432aff97,0xab9423a7,0xfc93a039,
        0x655b59c3,0x8f0ccc92,0xffeff47d,0x85845dd1,
        0x6fa87e4f,0xfe2ce6e0,0xa3014314,0x4e0811a1,
        0xf7537e82,0xbd3af235,0x2ad7d2bb,0xeb86d391,
    ];

    let orig_len_bits = (input.len() as u64) * 8;
    let mut msg = input.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&orig_len_bits.to_le_bytes());

    let (mut a0, mut b0, mut c0, mut d0): (u32, u32, u32, u32) =
        (0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476);

    for chunk in msg.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            m[i] = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
        }

        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);

        for i in 0..64 {
            let (f, g) = match i {
                0..=15  => ((b & c) | ((!b) & d), i),
                16..=31 => ((d & b) | ((!d) & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _       => (c ^ (b | (!d)), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(K[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(S[i]));
        }

        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut result = [0u8; 16];
    result[0..4].copy_from_slice(&a0.to_le_bytes());
    result[4..8].copy_from_slice(&b0.to_le_bytes());
    result[8..12].copy_from_slice(&c0.to_le_bytes());
    result[12..16].copy_from_slice(&d0.to_le_bytes());
    result
}

#[cfg(test)]
mod tests {
    use super::md5_hex;

    #[test]
    fn test_md5_known_values() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"hello"), "5d41402abc4b2a76b9719d911017c592");
    }
}
