use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeadIntent {
    #[serde(rename = "looking_for_tour")]
    LookingForTour,
    #[serde(rename = "finding_companions")]
    FindingCompanions,
    #[serde(rename = "hiring_guide")]
    HiringGuide,
    #[serde(rename = "hiring_porter")]
    HiringPorter,
    #[serde(rename = "route_advice")]
    RouteAdvice,
    #[serde(rename = "other")]
    Other,
}

impl ToString for LeadIntent {
    fn to_string(&self) -> String {
        match self {
            LeadIntent::LookingForTour => "looking_for_tour".to_string(),
            LeadIntent::FindingCompanions => "finding_companions".to_string(),
            LeadIntent::HiringGuide => "hiring_guide".to_string(),
            LeadIntent::HiringPorter => "hiring_porter".to_string(),
            LeadIntent::RouteAdvice => "route_advice".to_string(),
            LeadIntent::Other => "other".to_string(),
        }
    }
}

impl From<&str> for LeadIntent {
    fn from(s: &str) -> Self {
        match s {
            "looking_for_tour" => LeadIntent::LookingForTour,
            "finding_companions" => LeadIntent::FindingCompanions,
            "hiring_guide" => LeadIntent::HiringGuide,
            "hiring_porter" => LeadIntent::HiringPorter,
            "route_advice" => LeadIntent::RouteAdvice,
            _ => LeadIntent::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewStatus {
    #[serde(rename = "PENDING")]
    Pending,
    #[serde(rename = "APPROVED")]
    Approved,
    #[serde(rename = "REJECTED")]
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Lead {
    pub id: i64,
    pub post_id: i64,
    pub intent: String,
    pub confidence: f32,
    pub reason: String,
    pub rule_version: String,
    pub model_version: Option<String>,
    pub review_status: String,
    pub location: Option<String>,
    pub requested_date: Option<String>,
    pub summary: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewLead {
    pub post_id: i64,
    pub intent: String,
    pub confidence: f32,
    pub reason: String,
    pub rule_version: String,
    pub model_version: Option<String>,
    pub review_status: String,
    pub location: Option<String>,
    pub requested_date: Option<String>,
    pub summary: String,
}
