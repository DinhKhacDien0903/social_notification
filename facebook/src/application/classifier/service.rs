use std::sync::Arc;
use tracing::{info, warn};

use crate::application::interfaces::AiGateway;
use crate::domain::NewLead;
use super::rule::{RuleClassifier, RuleResult};

pub struct LeadClassificationOutput {
    pub is_lead: bool,
    pub lead: Option<NewLead>,
}

pub struct ClassifierService {
    rule_classifier: Arc<RuleClassifier>,
    ai_gateway: Arc<dyn AiGateway>,
}

impl ClassifierService {
    pub fn new(rule_classifier: Arc<RuleClassifier>, ai_gateway: Arc<dyn AiGateway>) -> Self {
        Self {
            rule_classifier,
            ai_gateway,
        }
    }

    pub async fn evaluate_post(&self, post_id: i64, text: &str) -> LeadClassificationOutput {
        // Tầng 1: Rule Taxonomy
        let rule_res = self.rule_classifier.classify(text);

        match rule_res {
            RuleResult::Positive {
                intent,
                confidence,
                reason,
                location,
            } => {
                info!("Post {} classified POSITIVE by Rule Taxonomy", post_id);
                LeadClassificationOutput {
                    is_lead: true,
                    lead: Some(NewLead {
                        post_id,
                        intent: intent.to_string(),
                        confidence,
                        reason,
                        rule_version: "v3.0.0-rule".to_string(),
                        model_version: None,
                        review_status: "APPROVED".to_string(),
                        location,
                        requested_date: None,
                        summary: format!("Lead phát hiện tự động bởi Rule: {:?}", intent),
                    }),
                }
            }
            RuleResult::Negative { reason } => {
                info!("Post {} rejected by Rule: {}", post_id, reason);
                LeadClassificationOutput {
                    is_lead: false,
                    lead: None,
                }
            }
            RuleResult::Uncertain { location, reason } => {
                info!("Post {} UNCERTAIN by Rule ({}), delegating to AI Gateway...", post_id, reason);

                match self.ai_gateway.classify_post(text).await {
                    Ok(ai_res) => {
                        if ai_res.is_lead && ai_res.confidence >= 0.70 {
                            LeadClassificationOutput {
                                is_lead: true,
                                lead: Some(NewLead {
                                    post_id,
                                    intent: ai_res.intent,
                                    confidence: ai_res.confidence,
                                    reason: format!("AI: {}", ai_res.reason),
                                    rule_version: "v3.0.0-rule".to_string(),
                                    model_version: Some("gemini-1.5-flash".to_string()),
                                    review_status: "APPROVED".to_string(),
                                    location: ai_res.location.or(location),
                                    requested_date: ai_res.requested_date,
                                    summary: ai_res.summary,
                                }),
                            }
                        } else {
                            LeadClassificationOutput {
                                is_lead: false,
                                lead: None,
                            }
                        }
                    }
                    Err(err) => {
                        warn!("AI Gateway failed for post {}: {}. Fallback to manual review.", post_id, err);
                        // Fallback an toàn: nếu có địa danh thì đẩy vào PENDING để review thủ công
                        LeadClassificationOutput {
                            is_lead: true,
                            lead: Some(NewLead {
                                post_id,
                                intent: "other".to_string(),
                                confidence: 0.60,
                                reason: format!("Fallback do AI Gateway lỗi: {}", err),
                                rule_version: "v3.0.0-rule".to_string(),
                                model_version: None,
                                review_status: "PENDING".to_string(),
                                location,
                                requested_date: None,
                                summary: "Cần duyệt thủ công do AI Gateway gặp sự cố".to_string(),
                            }),
                        }
                    }
                }
            }
        }
    }
}
