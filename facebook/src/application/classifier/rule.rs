use regex::Regex;
use unicode_normalization::UnicodeNormalization;

use crate::domain::LeadIntent;

#[derive(Debug, Clone)]
pub enum RuleResult {
    Positive {
        intent: LeadIntent,
        confidence: f32,
        reason: String,
        location: Option<String>,
        requested_date: Option<String>,
        summary: String,
    },
    Negative {
        reason: String,
    },
    Uncertain {
        location: Option<String>,
        requested_date: Option<String>,
        reason: String,
    },
}

pub struct RuleClassifier {
    seller_patterns: Vec<Regex>,
    sharing_patterns: Vec<Regex>,
    high_intent_patterns: Vec<(Regex, LeadIntent)>,
    locations: Vec<(&'static str, &'static str)>, // (canonical_display, search_keyword_normalized)
    date_patterns: Vec<Regex>,
    question_patterns: Vec<Regex>,
    heuristic_porter_pattern: Regex,
    heuristic_guide_pattern: Regex,
    heuristic_tour_pattern: Regex,
    heuristic_tour_cue_pattern: Regex,
    heuristic_companion_pattern: Regex,
    heuristic_companion_cue_pattern: Regex,
}

impl Default for RuleClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleClassifier {
    pub fn new() -> Self {
        let seller_regexes = vec![
            // ── 1. Người bán tour / Tour operator / Tour leader quảng cáo ──
            r"mo\s+ban\s+tour\b",
            r"lich\s+khoi\s+hanh\b",
            r"\bkhoi\s+hanh\b",
            r"gia\s+chi\s+tu\b",
            r"combo\s+du\s+lich\b",
            r"tour\s+tron\s+goi\b",
            r"gia\s+tron\s+goi\b",
            r"chi\s+phi\s+tron\s+goi\b",
            r"bao\s+tron\s+goi\b",
            r"gia\s+tour\b",
            r"bang\s+gia\b",
            r"(?:inbox|ib|cmt|comment)\s+(?:de\s+)?(?:nhan\s+)?bao\s+gia\b",
            r"(?:inbox|ib|cmt|comment)\s+(?:em|minh)?\s*(?:gui|de\s+nhan|de\s+lay)\s+lich\s+trinh",
            r"gui\s+lich\s+trinh\s+chi\s+tiet",
            r"lich\s+trinh\s+chi\s+tiet\b",
            r"lich\s+trinh\s+cu\s+the\b",
            r"\b\d+n\d+d\b", // 3N2Đ, 2N1Đ
            r"\b\d+\s*ngay\s*\d+\s*dem\b", // 3 ngày 2 đêm
            r"dich\s+vu\s+bao\s+gom\b",
            r"chi\s+phi\s+bao\s+gom\b",
            r"chua\s+bao\s+gom\b",
            r"da\s+bao\s+gom\b",
            r"bao\s+gom\s*:",
            r"hen\s+nhau\s+chinh\s+phuc\b",
            r"chinh\s+phuc\s+noc\s+nha\b",
            r"san\s+bien\s+may\s+bong\s+benh\b",
            r"ban\s+da\s+san\s+sang\s+(?:cho\s+chuyen|chinh\s+phuc|len\s+xe)",
            r"con\s+\d+\s+slot\b",
            r"nhanh\s+tay\s+dang\s+ky\b",
            r"tuyen\s+ctv\b",
            r"tuyen\s+dai\s+ly\b",
            r"uu\s+dai\s+dac\s+biet\b",
            r"gia\s+uu\s+dai\b",
            r"tro\s+gia\b",
            r"chuyen\s+khoan\s+coc\b",
            r"dat\s+coc\b",
            r"giu\s+cho\b",
            r"link\s+dang\s+ky\b",
            r"\b(?:ngay|day)\s*[1-5]\s*:",
            r"\bn[1-5]\s*:",
            r"lien\s+he\s+(?:hotline|zalo|sdt|phone)\b",
            r"\bhotline\s*:",
            r"\bzalo\s*:",
            r"\bsdt\s*:",
            r"\bcall/zalo\b",

            // ── 2. Dịch vụ xe du lịch / Cho thuê xe ──
            r"xe\s+du\s+lich\b",
            r"thue\s+xe\b",
            r"cho\s+thue\s+xe\b",
            r"xe\s+hop\s+dong\b",
            r"xe\s+tien\s+chuyen\b",
            r"xe\s+ghep\b",
            r"nha\s+xe\b",
            r"xe\s+(?:tu\s+)?\d+\s*(?:-|–|\s+den\s+)\s*\d+\s*cho\b",
            r"xe\s+\d+\s*cho\b",
            r"#xedulich\b",
            r"#thuexe\b",
            r"#xeghep\b",
            r"lai\s+xe\b",
            r"tai\s+xe\b",
            r"xe\s+don\s+tien\b",

            // ── 3. Bán đồ / Pass đồ / Đồ dùng cá nhân ──
            r"thanh\s+ly\s+do\b",
            r"thanh\s+ly\b",
            r"pass\s+lai\b",
            r"pass\s+(?:nhanh|gap|re|do|giay|balo|ao|quan|gang|gay|den|leu|tui|ve|tour)\b",
            r"nhuong\s+lai\b",
            r"cho\s+thue\s+leu\b",
            r"cho\s+thue\s+do\b",
            r"ban\s+(?:balo|giay|ao|quan|leu|tui\s+ngu|gay|den|bep)\b",
            r"muon\s+muc\s+em\s+no\b",
            r"muc\s+em\s+no\b",
            r"size\s+\d{2}\b",
            r"bao\s+check\b",
            r"new\s+tag\b",
            r"full\s+box\b",
            r"chua\s+(?:di|dung)\s+lan\s+nao\b",
            r"mua\s+(?:ve\s+)?khong\s+(?:dung|hop|vua|kip)\b",
            r"(?:gia|pass|tl)\s*[:=]?\s*\d+[kK]\b",

            // ── 4. Nhóm Phượt tự túc / Chia sẻ chi phí (Không mua tour) ──
            r"cam\s+pu\s+chia\b",
            r"cam-pu-chia\b",
            r"campuchia\b",
            r"chi\s+phi\s+cam\b",
            r"share\s+chi\s+phi\b",
            r"chia\s+chi\s+phi\b",
            r"xe\s+tu\s+lai\b",
            r"tu\s+lai\b",
            r"xe\s+may\s+tu\s+tuc\b",
            r"tu\s+tuc\s+(?:chi\s+phi|bang\s+xe|xe\s+may)\b",
            r"team\s+tu\s+tuc\b",
        ];

        let seller_patterns = seller_regexes
            .into_iter()
            .map(|p| Regex::new(&format!(r"(?i){}", p)).unwrap())
            .collect();

        let sharing_regexes = vec![
            r"chia\s+se\s+hanh\s+trinh\b",
            r"trai\s+nghiem\s+cua\s+minh\b",
            r"gui\s+ca\s+nha\s+bo\s+anh\b",
            r"anh\s+dep\s+chuyen\s+di\b",
            r"nhat\s+ky\s+hanh\s+trinh\b",
            r"ky\s+niem\s+chuyen\s+di\b",
            r"trai\s+nghiem\s+tuyet\s+voi\b",
            r"that\s+lac\b",
            r"danh\s+roi\b",
            r"tim\s+do\b",
            r"roi\s+do\b",
            r"a\s+beautiful\s+day\b",
            r"what\s+an\s+amazing\s+day\b",
        ];

        let sharing_patterns = sharing_regexes
            .into_iter()
            .map(|p| Regex::new(&format!(r"(?i){}", p)).unwrap())
            .collect();

        let high_intent_tuples = vec![
            // ── LookingForTour ──────────────────────────────────────────────
            (r"(?:can|muon|mun|tim|hoi|xin|dat|kiem)\s+(?:1\s+vai\s+|vai\s+)?tour\b", LeadIntent::LookingForTour),
            (r"co\s+tour\s+(?:nao|di|trekking|leo)\b", LeadIntent::LookingForTour),
            (r"co\s+(?:ben|ai|doan|cong\s+ty|page)\s+nao\s+(?:co|nhan|to\s+chuc|con)\s+tour\b", LeadIntent::LookingForTour),
            (r"co\s+(?:ai\s+co\s+|ben\s+nao\s+co\s+)?lich\s+(?:trekking|di|leo)\b", LeadIntent::LookingForTour),
            (r"lich\s+trekking.*(?:ko|khong|k\b|hong|\?)", LeadIntent::LookingForTour),
            (r"(?:ben|page)\s+nao\s+con\s+slot\b", LeadIntent::LookingForTour),
            (r"tu\s+van\s+tour\b", LeadIntent::LookingForTour),
            (r"tu\s+van\s+giup\s+em\s+(?:tour|cung)\b", LeadIntent::LookingForTour),
            (r"co\s+ai\s+ban\s+tour\b", LeadIntent::LookingForTour),
            (r"tour\s+cho\s+newbie\b", LeadIntent::LookingForTour),
            (r"dang\s+k[iy]\s+(?:theo\s+)?tour\b", LeadIntent::LookingForTour),
            (r"(?:cho\s+e|cho\s+em|cho\s+minh)\s+dki\s+\d+\s+suat\b", LeadIntent::LookingForTour),
            (r"(?:team|doan|tour)\s+nao\s+co\s+slot\s+cho\s+(?:e|em|minh)\s+dki\b", LeadIntent::LookingForTour),
            (r"tim\s+tour\s+[\w\s]{1,30}\s+(?:cmt|inbox|ib|tu\s+van)\b", LeadIntent::LookingForTour),
            (r"co\s+ai\s+co\s+lich\s+trekking\b", LeadIntent::LookingForTour),
            (r"muon\s+di\s+[\w\s]{1,20}\s+ben\s+nao\s+con\s+slot\b", LeadIntent::LookingForTour),
            (r"(?:can|tim)\s+tour\s+ghep\b", LeadIntent::LookingForTour),
            (r"xin\s+review\s+(?:ben|cong\s+ty)\s+tour\b", LeadIntent::LookingForTour),

            // ── HiringPorter ────────────────────────────────────────────────
            (r"(?:can|tim|thue|xin\s+so|xin\s+sdt|xin\s+contact|lien\s+he|gioi\s+thieu)\s+(?:\w+\s+){0,4}(?:porter|poster|poter)\b", LeadIntent::HiringPorter),
            (r"\bporter.*(?:uy\s+tin|nhiet\s+tinh|fly\s*cam|gia|ib)", LeadIntent::HiringPorter),
            (r"(?:can|tim|thue|dat|xin\s+so|xin\s+sdt)\s+lan\s+(?:nghi)?\b", LeadIntent::HiringPorter),
            (r"thue\s+lan\s+(?:tren|o)\s+[\w\s]{1,30}\s+(?:can\s+lien\s+he|lien\s+he)", LeadIntent::HiringPorter),
            (r"(?:xin\s+thong\s+tin|xin\s+sdt)\s+porter\b", LeadIntent::HiringPorter),

            // ── HiringGuide ─────────────────────────────────────────────────
            (r"(?:can|tim|thue|xin\s+so|xin\s+sdt|xin\s+contact)\s+(?:\w+\s+){0,4}(?:guide|huong\s+dan\s+vien|nguoi\s+dan\s+duong)\b", LeadIntent::HiringGuide),

            // ── FindingCompanions ───────────────────────────────────────────
            (r"ghep\s+(?:doan|nhom|tour|team|cung)\b", LeadIntent::FindingCompanions),
            (r"(?:cho\s+e|cho\s+em|cho\s+minh)\s+(?:join|ghep|theo|di\s+cung)\s+(?:doan|nhom|tour|team)\b", LeadIntent::FindingCompanions),
            (r"co\s+(?:doan|nhom|team)\s+nao\s+(?:leo|di|trekking).*(?:cho\s+(?:e|em|minh)\s+(?:ghep|ke|join))", LeadIntent::FindingCompanions),
            (r"(?:tim|kiem)\s+(?:team|nhom|dong\s+doi|ban)\s+leo\s+[\w\s]{1,25}\s+(?:dk|dang\s+ky|dki)\s+\d+\s+slot\b", LeadIntent::FindingCompanions),

            // ── RouteAdvice ─────────────────────────────────────────────────
            (r"xin\s+kinh\s+nghiem\b", LeadIntent::RouteAdvice),
            (r"chia\s+se\s+kinh\s+nghiem\s+voi\b", LeadIntent::RouteAdvice),
            (r"hoi\s+kinh\s+nghiem\b", LeadIntent::RouteAdvice),
            (r"tu\s+van\s+(?:cung|duong|lich\s+trinh)\b", LeadIntent::RouteAdvice),
            (r"xin\s+lich\s+trinh\b", LeadIntent::RouteAdvice),
            (r"thoi\s+tiet.*(?:the\s+nao|mua\s+khong|mua\s+ko|sao\s+roi|\?)", LeadIntent::RouteAdvice),
            (r"chi\s+phi.*(?:bao\s+nhieu|het\s+bao\s+nhieu|the\s+nao)", LeadIntent::RouteAdvice),
            (r"(?:cung|duong|leo).*(?:co\s+kho\s+khong|co\s+kho\s+ko|the\s+nao|di\s+may\s+ngay)", LeadIntent::RouteAdvice),
            (r"(?:di\s+toi\s+diem\s+nao|di\s+cung\s+nao|di\s+dau\s+dep)\b", LeadIntent::RouteAdvice),
            (r"co\s+the\s+ket\s+hop\s+di\b", LeadIntent::RouteAdvice),
            (r"co\s+bac\s+nao\s+di\s+cung.*chua", LeadIntent::RouteAdvice),
        ];

        let high_intent_patterns = high_intent_tuples
            .into_iter()
            .map(|(p, intent)| (Regex::new(&format!(r"(?i){}", p)).unwrap(), intent))
            .collect();

        let locations = vec![
            ("Kỳ Quan San", "ky quan san"),
            ("Bạch Mộc Lương Tử", "bach moc luong tu"),
            ("Bạch Mộc Lương Tử", "bach moc"),
            ("Tà Chì Nhù", "ta chi nhu"),
            ("Tà Chì Nhù", "mo chi"),
            ("Tà Xùa", "ta xua"),
            ("Lùng Cúng", "lung cung"),
            ("Lùng Cúng", "lùng cúng"),
            ("Nhìu Cồ San", "nhiu co san"),
            ("Phu Sa Phìn", "phu sa phin"),
            ("Phu Sa Phìn", "phun sa phin"),
            ("Phu Sa Phìn", "phu tha phin"),
            ("Samu", "samu"),
            ("Samu", "u bo"),
            ("Samu", "samu u bo"),
            ("Hàm Lợn", "ham lon"),
            ("Vườn quốc gia Ba Vì", "ba vi"),
            ("Vườn quốc gia Ba Vì", "vuon quoc gia ba vi"),
            ("Đà Giang", "da giang"),
            ("Hồ Mắt Ngọc", "ho mat ngoc"),
            ("Tú Lệ", "tu le"),
        ];

        let date_patterns = vec![
            Regex::new(r"(?i)\b\d{1,2}\s*[-–/]\s*\d{1,2}\s*[/.-]\s*\d{1,2}\b").unwrap(),
            Regex::new(r"(?i)\b\d{1,2}\s*[-–/]\s*\d{1,2}\s+thang\s+\d{1,2}\b").unwrap(),
            Regex::new(r"(?i)\b\d{1,2}\s*[/.-]\s*\d{1,2}\b").unwrap(),
            Regex::new(r"(?i)\bthang\s+\d{1,2}\b").unwrap(),
            Regex::new(r"(?i)\btet\s+(?:tay|al|ta|nguyen\s+dan)\b").unwrap(),
            Regex::new(r"(?i)\bcuoi\s+tuan\s+(?:nay|toi)\b").unwrap(),
            Regex::new(r"(?i)\btuan\s+sau\b").unwrap(),
        ];

        let question_patterns = vec![
            Regex::new(r"\?").unwrap(),
            Regex::new(r"(?i)\b(?:ko|khong|k|khum|hong)\b\??").unwrap(),
            Regex::new(r"(?i)\b(?:hoi|xin|tu\s+van|muon|mun|co\s+ai|ai\s+co|ai\s+biet|bac\s+nao|co\s+doan\s+nao)\b").unwrap(),
        ];

        let heuristic_porter_pattern = Regex::new(r"(?i)\b(?:porter|poster|poter|lan\s+nghi|thue\s+lan|dat\s+lan)\b").unwrap();
        let heuristic_guide_pattern = Regex::new(r"(?i)\b(?:guide|huong\s+dan\s+vien|dan\s+duong)\b").unwrap();
        let heuristic_tour_pattern = Regex::new(r"(?i)\b(?:tour|slot|lich\s+trekking|lich\s+di|lich\s+leo)\b").unwrap();
        let heuristic_tour_cue_pattern = Regex::new(r"(?i)\b(?:co\s+ai|ai\s+co|ben\s+nao|tim|can|hoi|xin|con|muon|mun|dki|dang\s+ky)\b").unwrap();
        let heuristic_companion_pattern = Regex::new(r"(?i)\b(?:ghep\s+doan|ghep\s+nhom|ghep\s+team|di\s+chung|di\s+cung|leo\s+cung|dong\s+doi)\b").unwrap();
        let heuristic_companion_cue_pattern = Regex::new(r"(?i)\b(?:co\s+ai|cho\s+(?:e|em|minh)|ai\s+di|tim|can|keo)\b").unwrap();

        Self {
            seller_patterns,
            sharing_patterns,
            high_intent_patterns,
            locations,
            date_patterns,
            question_patterns,
            heuristic_porter_pattern,
            heuristic_guide_pattern,
            heuristic_tour_pattern,
            heuristic_tour_cue_pattern,
            heuristic_companion_pattern,
            heuristic_companion_cue_pattern,
        }
    }

    /// Khử dấu tiếng Việt sang ASCII không dấu
    pub fn strip_accents(s: &str) -> String {
        s.nfd()
            .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
            .collect::<String>()
            .to_lowercase()
            .replace('đ', "d")
            .replace('Đ', "d")
    }

    /// Trích xuất ngày/tháng/dịp khởi hành từ nội dung bài viết
    pub fn extract_date(&self, normalized: &str) -> Option<String> {
        for pat in &self.date_patterns {
            if let Some(mat) = pat.find(normalized) {
                return Some(mat.as_str().trim().to_string());
            }
        }
        None
    }

    pub fn classify(&self, raw_text: &str) -> RuleResult {
        let normalized = Self::strip_accents(raw_text);

        // 1. Kiểm tra Người bán (Seller)
        for pat in &self.seller_patterns {
            if pat.is_match(&normalized) {
                return RuleResult::Negative {
                    reason: "Khớp từ khóa người bán/quảng cáo tour/sản phẩm".to_string(),
                };
            }
        }

        // 2. Tìm địa danh trekking
        let mut detected_location: Option<String> = None;
        for (canonical, loc_norm) in &self.locations {
            if normalized.contains(loc_norm) {
                detected_location = Some(canonical.to_string());
                break;
            }
        }

        // Kiểm tra ngữ cảnh trekking tổng quát nếu chưa bắt được địa danh cụ thể
        let has_trekking_context = detected_location.is_some()
            || normalized.contains("trekking")
            || normalized.contains("leo nui")
            || normalized.contains(" trek ")
            || normalized.starts_with("trek ")
            || normalized.ends_with(" trek")
            || normalized.contains("chinh phuc dinh")
            || normalized.contains("cung ");

        // Nếu hoàn toàn không liên quan leo núi hoặc địa danh -> Loại bỏ bài viết ngoài lề
        if !has_trekking_context {
            return RuleResult::Negative {
                reason: "Không chứa địa danh trekking hoặc từ khóa liên quan leo núi".to_string(),
            };
        }

        // 3. Trích xuất thời gian dự kiến (nếu có)
        let requested_date = self.extract_date(&normalized);

        // Chuẩn bị tóm tắt ngắn 1 dòng (tối đa 140 ký tự)
        let summary_snippet = raw_text
            .replace(['\r', '\n'], " ")
            .trim()
            .chars()
            .take(140)
            .collect::<String>();

        // 4. Kiểm tra Bài chia sẻ/Review thuần túy hoặc đồ vật thất lạc
        let is_pure_sharing = self.sharing_patterns.iter().any(|p| p.is_match(&normalized));
        let has_question = self.question_patterns.iter().any(|p| p.is_match(&normalized));

        if is_pure_sharing && !has_question {
            return RuleResult::Negative {
                reason: "Khớp từ khóa chia sẻ hành trình/recap/đồ thất lạc không mang ý định hỏi mua".to_string(),
            };
        }

        // 5. Kiểm tra Ý định mua cao theo mẫu quy tắc (High Intent Patterns)
        for (pat, intent) in &self.high_intent_patterns {
            if pat.is_match(&normalized) {
                return RuleResult::Positive {
                    intent: intent.clone(),
                    confidence: 0.95,
                    reason: format!("Khớp mẫu ý định mua cao: {:?}", intent),
                    location: detected_location,
                    requested_date,
                    summary: summary_snippet,
                };
            }
        }

        // 6. Kiểm tra Heuristic: Bài viết có ngữ cảnh trekking + câu hỏi về dịch vụ cụ thể
        if has_trekking_context && has_question {
            if self.heuristic_porter_pattern.is_match(&normalized) {
                return RuleResult::Positive {
                    intent: LeadIntent::HiringPorter,
                    confidence: 0.85,
                    reason: "Phát hiện câu hỏi thắc mắc tìm porter hoặc thuê lán".to_string(),
                    location: detected_location,
                    requested_date,
                    summary: summary_snippet,
                };
            }

            if self.heuristic_guide_pattern.is_match(&normalized) {
                return RuleResult::Positive {
                    intent: LeadIntent::HiringGuide,
                    confidence: 0.85,
                    reason: "Phát hiện câu hỏi thắc mắc tìm hướng dẫn viên / guide".to_string(),
                    location: detected_location,
                    requested_date,
                    summary: summary_snippet,
                };
            }

            if self.heuristic_tour_pattern.is_match(&normalized)
                && self.heuristic_tour_cue_pattern.is_match(&normalized)
            {
                return RuleResult::Positive {
                    intent: LeadIntent::LookingForTour,
                    confidence: 0.85,
                    reason: "Phát hiện câu hỏi thắc mắc tìm tour hoặc lịch khởi hành".to_string(),
                    location: detected_location,
                    requested_date,
                    summary: summary_snippet,
                };
            }

            if self.heuristic_companion_pattern.is_match(&normalized)
                && self.heuristic_companion_cue_pattern.is_match(&normalized)
            {
                return RuleResult::Positive {
                    intent: LeadIntent::FindingCompanions,
                    confidence: 0.85,
                    reason: "Phát hiện câu hỏi tìm ghép đoàn hoặc bạn đồng hành".to_string(),
                    location: detected_location,
                    requested_date,
                    summary: summary_snippet,
                };
            }
        }

        // 7. Nếu có nhắc đến địa danh nhưng ý định chưa rõ ràng -> Chuyển tầng 2 (AI Gateway)
        RuleResult::Uncertain {
            location: detected_location,
            requested_date,
            reason: "Có đề cập đến trekking nhưng ý định chưa rõ ràng, cần AI thẩm định".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reject_tour_sellers() {
        let classifier = RuleClassifier::new();

        // Ảnh 2 & 4: Bài bán tour Kỳ Quan San
        let post1 = "KY QUAN SAN | 21 – 23/11 | 3N2Đ THÁNG 11 – HẸN NHAU CHINH PHỤC NÓC NHÀ BẠCH MỘC LƯƠNG TỬ! Ky Quan San mùa này có gì mà khiến dân trekking mê mẩn đến vậy? Săn biển mây bồng bềnh giữa núi rừng Tây Bắc. Băng qua những cánh rừng nguyên sinh tuyệt đẹp. Chinh phục sống lưng khủng long và đón bình minh trên biển mây. Check-in đỉnh Bạch Mộc Lương Tử ở độ cao 3.046m. Một hành trình 3 ngày 2 đêm để thử thách bản thân, tận hưởng thiên nhiên và lưu lại những khoảnh khắc đáng nhớ. Bạn đã sẵn sàng cho chuyến trekking ...";
        let res1 = classifier.classify(post1);
        assert!(matches!(res1, RuleResult::Negative { .. }), "Bài bán tour phải bị loại trừ (Negative)");

        // Ảnh 6: Tour leader gom khách kèm câu gửi lịch trình
        let post2 = "TÌM THÀNH VIÊN LEO KY QUAN SAN Em tìm đồng đội leo Ky Quan San 02 ngày 01 đêm (13,14/12 - Chủ nhật, thứ 2) ace nào quan tâm ib em gửi lịch trình chi tiết ạ";
        let res2 = classifier.classify(post2);
        assert!(matches!(res2, RuleResult::Negative { .. }), "Bài tour leader gom khách phải bị loại trừ (Negative)");
    }

    #[test]
    fn test_reject_diy_and_cost_sharing() {
        let classifier = RuleClassifier::new();

        // Ảnh 3: Phượt tự túc xe tự lái share tiền campuchia
        let post = "Tìm thêm người leo combo Sơn Nhạc Chung, Tà xùa hồ cuối tuần này 7-8/11. Tối thứ 6 xuất phát, đi xe tự lái đến đèo Ô quy hồ. 7h leo Sơn Nhạc Chung, sang Fancipan và đi cáp treo về. Tối ngủ Tam Đường. Sáng chủ nhật 7h leo Tà xùa hồ, chiều về chèo sup hồ Thác bạc. Chiều tối ngắm hoàng hôn và ăn buffet Hải Yến, về Hn. Chi phí căm pu chia chân núi. BD Ẩn bớt";
        let res = classifier.classify(post);
        assert!(matches!(res, RuleResult::Negative { .. }), "Bài phượt tự túc chia chi phí phải bị loại trừ (Negative)");
    }

    #[test]
    fn test_reject_pass_items_and_resale() {
        let classifier = RuleClassifier::new();

        // Ảnh 5: Pass giày trekking
        let post = "Em mua đôi này size 40 (38 tăng 2 size) về để đi trekking mà không về kịp, em đi rồi nó mới về. Sẵn HN chị em nào sắp đi muốn múc em nó hông ạ. bao xinh bao check, 500 huhu";
        let res = classifier.classify(post);
        assert!(matches!(res, RuleResult::Negative { .. }), "Bài pass đồ cá nhân phải bị loại trừ (Negative)");
    }

    #[test]
    fn test_reject_car_rental_services() {
        let classifier = RuleClassifier::new();

        // Ảnh 7: Cho thuê xe du lịch
        let post = "TÂY BẮC KHÔNG CHỈ CÓ MÂY, MÀ CÒN CÓ NHỮNG HÀNH TRÌNH ĐÁNG NHỚ! Một chuyến đi đẹp cần một người bạn đồng hành tin cậy. Xe du lịch Gia Bảo sẵn sàng cùng bạn chinh phục những cung đường Tây Bắc hùng vĩ! Từ những con đèo uốn lượn đến biển mây bồng bềnh, cứ xách vali lên và đi — việc có xe, để Gia Bảo lo! Xe du lịch từ 5-45 chỗ Liên hệ thuê xe: 0326233300 Tây Bắc đang chờ — bạn đã sẵn sàng lên xe chưa? #XeDuLichGiaBao #TayBac #ThueXeDuLich #SanMayTayBac";
        let res = classifier.classify(post);
        assert!(matches!(res, RuleResult::Negative { .. }), "Bài cho thuê xe phải bị loại trừ (Negative)");
    }

    #[test]
    fn test_accept_genuine_leads() {
        let classifier = RuleClassifier::new();

        // Khách tìm tour Ba Vì
        let post1 = "Em tìm tour Ba Vì , các bác cmt em tham khảo nhé ạ";
        let res1 = classifier.classify(post1);
        match res1 {
            RuleResult::Positive { intent, location, .. } => {
                assert_eq!(intent, LeadIntent::LookingForTour);
                assert_eq!(location.as_deref(), Some("Vườn quốc gia Ba Vì"));
            }
            other => panic!("Expected Positive LookingForTour, got {:?}", other),
        }

        // Khách tìm tour Kỳ Quan San
        let post2 = "Ky Quan San Em tìm tour Ky Quan San 21-22-23-24/11 ạ. Team nào có slot cho em dki 2 suất ạ";
        let res2 = classifier.classify(post2);
        match res2 {
            RuleResult::Positive { intent, location, .. } => {
                assert_eq!(intent, LeadIntent::LookingForTour);
                assert_eq!(location.as_deref(), Some("Kỳ Quan San"));
            }
            other => panic!("Expected Positive LookingForTour, got {:?}", other),
        }

        // Khách tìm porter Kỳ Quan San
        let post3 = "Em xin thông tin porter dẫn leo Ky Quan San ạ";
        let res3 = classifier.classify(post3);
        match res3 {
            RuleResult::Positive { intent, location, .. } => {
                assert_eq!(intent, LeadIntent::HiringPorter);
                assert_eq!(location.as_deref(), Some("Kỳ Quan San"));
            }
            other => panic!("Expected Positive HiringPorter, got {:?}", other),
        }

        // Khách muốn thuê lán Tà Chì Nhù
        let post4 = "Mình leo tự túc muốn thuê lán trên Tà Chì Nhù cần liên hệ";
        let res4 = classifier.classify(post4);
        match res4 {
            RuleResult::Positive { intent, location, .. } => {
                assert_eq!(intent, LeadIntent::HiringPorter);
                assert_eq!(location.as_deref(), Some("Tà Chì Nhù"));
            }
            other => panic!("Expected Positive HiringPorter, got {:?}", other),
        }
    }
}
