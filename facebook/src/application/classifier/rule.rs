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
}

impl Default for RuleClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleClassifier {
    pub fn new() -> Self {
        let seller_regexes = vec![
            r"mo ban tour",
            r"lich khoi hanh",
            r"khoi hanh\b",
            r"gia chi tu",
            r"combo du lich",
            r"inbox de nhan bao gia",
            r"inbox nhan bao gia",
            r"lien he hotline",
            r"hotline:",
            r"zalo:",
            r"con \d+ slot",
            r"nhanh tay dang ky",
            r"tuyen ctv",
            r"tuyen dai ly",
            r"thanh ly do",
            r"pass lai do",
            r"cho thue leu",
            r"ban balo",
            r"ban giay",
            r"uu dai dac biet",
            r"gia uu dai",
            r"tro gia",
            r"xe 4.*45 cho",
            r"bao tron goi",
        ];

        let seller_patterns = seller_regexes
            .into_iter()
            .map(|p| Regex::new(&format!(r"(?i){}", p)).unwrap())
            .collect();

        let sharing_regexes = vec![
            r"chia se hanh trinh",
            r"trai nghiem cua minh",
            r"gui ca nha bo anh",
            r"anh dep chuyen di",
            r"nhat ky hanh trinh",
            r"ky niem chuyen di",
            r"trai nghiem tuyet voi",
            r"that lac",
            r"danh roi",
            r"tim do",
            r"roi do",
            r"a beautiful day",
            r"what an amazing day",
        ];

        let sharing_patterns = sharing_regexes
            .into_iter()
            .map(|p| Regex::new(&format!(r"(?i){}", p)).unwrap())
            .collect();

        let high_intent_tuples = vec![
            // ── LookingForTour ──────────────────────────────────────────────
            (r"(?:can|muon|mun|tim|hoi|xin|dat|kiem)\s+(?:1\s+vai\s+|vai\s+)?tour", LeadIntent::LookingForTour),
            (r"co\s+tour\s+(?:nao|di|trekking|leo)", LeadIntent::LookingForTour),
            (r"co\s+(?:ben|ai|doan|cong\s+ty|page)\s+nao\s+(?:co|nhan|to\s+chuc|con)\s+tour", LeadIntent::LookingForTour),
            (r"co\s+(?:ai\s+co\s+|ben\s+nao\s+co\s+)?lich\s+(?:trekking|di|leo)", LeadIntent::LookingForTour),
            (r"lich\s+trekking.*(?:ko|khong|k\b|hong|a\b|\?)", LeadIntent::LookingForTour),
            (r"(?:ben|ai|tour)\s+nao\s+con\s+slot", LeadIntent::LookingForTour),
            (r"tu\s+van\s+tour", LeadIntent::LookingForTour),
            (r"tu\s+van\s+giup\s+em", LeadIntent::LookingForTour),
            (r"co\s+ai\s+ban\s+tour", LeadIntent::LookingForTour),
            (r"tour\s+cho\s+newbie", LeadIntent::LookingForTour),
            (r"tour.*ve\s+trong\s+ngay", LeadIntent::LookingForTour),
            (r"co\s+cung\s+nao.*(?:gia\s+re|2tr|3tr|\d+k|\d+tr)", LeadIntent::LookingForTour),
            (r"pass\s+tour", LeadIntent::LookingForTour),
            (r"dang\s+k[iy]\s+(?:theo\s+)?tour", LeadIntent::LookingForTour),

            // ── HiringPorter ────────────────────────────────────────────────
            (r"(?:can|tim|thue|xin\s+so|xin\s+contact|lien\s+he|gioi\s+thieu)\s+(?:\w+\s+){0,4}(?:porter|poster|poter)", LeadIntent::HiringPorter),
            (r"porter.*(?:uy\s+tin|nhiet\s+tinh|fly\s*cam|gia|ib)", LeadIntent::HiringPorter),
            (r"(?:can|tim|thue|dat|xin\s+so)\s+lan\s+(?:nghi)?", LeadIntent::HiringPorter),

            // ── HiringGuide ─────────────────────────────────────────────────
            (r"(?:can|tim|thue|xin\s+so|xin\s+contact)\s+(?:\w+\s+){0,4}(?:guide|huong\s+dan\s+vien|nguoi\s+dan\s+duong)", LeadIntent::HiringGuide),

            // ── FindingCompanions ───────────────────────────────────────────
            (r"ghep\s+(?:doan|nhom|tour|team|cung)", LeadIntent::FindingCompanions),
            (r"(?:tim|kiem|can)\s+(?:them\s+)?(?:nhom|ban|dong\s+doi|dong\s+bon|nguoi|ca|team|thanh\s+vien)\s+(?:ghep|di\s+cung|di\s+chung|dong\s+hanh|leo|trekking)", LeadIntent::FindingCompanions),
            (r"tim\s+(?:dong\s+doi|dong\s+bon|ban\s+dong\s+hanh|team|ban\s+leo)", LeadIntent::FindingCompanions),
            (r"(?:tuyen|can|tim)\s+(?:them\s+)?(?:thanh\s+vien|ban|nguoi|dong\s+bon|dong\s+doi|slot|mam)", LeadIntent::FindingCompanions),
            (r"len\s+keo", LeadIntent::FindingCompanions),
            (r"cho\s+(?:e|em|minh)\s+(?:join|ghep|theo|di\s+cung)", LeadIntent::FindingCompanions),
            (r"co\s+(?:ai|bac\s+nao|ban\s+nao|doan\s+nao|nhom\s+nao|team\s+nao)\s+(?:di|leo|trekking|chinh\s+phuc|tham\s+gia).*(?:chung|cung|ko|khong|k\b|hong|chua|\?)", LeadIntent::FindingCompanions),
            (r"co\s+(?:doan|nhom|team)\s+nao\s+(?:leo|di|trekking)", LeadIntent::FindingCompanions),
            (r"co\s+ai\s+di\s+chung", LeadIntent::FindingCompanions),
            (r"co\s+ai\s+di\s+cung", LeadIntent::FindingCompanions),
            (r"co\s+ban\s+nao\s+(?:di|leo)\s+cung", LeadIntent::FindingCompanions),
            (r"team\s+tu\s+tuc.*(?:leo|di|thieu|tuyen)", LeadIntent::FindingCompanions),
            (r"lap\s+team.*(?:di|leo|cung)", LeadIntent::FindingCompanions),
            (r"co\s+keo\s+nao", LeadIntent::FindingCompanions),

            // ── RouteAdvice ─────────────────────────────────────────────────
            (r"xin\s+kinh\s+nghiem", LeadIntent::RouteAdvice),
            (r"chia\s+se\s+kinh\s+nghiem\s+voi", LeadIntent::RouteAdvice),
            (r"hoi\s+kinh\s+nghiem", LeadIntent::RouteAdvice),
            (r"tu\s+van\s+(?:cung|duong|lich\s+trinh)", LeadIntent::RouteAdvice),
            (r"xin\s+lich\s+trinh", LeadIntent::RouteAdvice),
            (r"thoi\s+tiet.*(?:the\s+nao|mua\s+khong|mua\s+ko|sao\s+roi|\?)", LeadIntent::RouteAdvice),
            (r"chi\s+phi.*(?:bao\s+nhieu|het\s+bao\s+nhieu|the\s+nao)", LeadIntent::RouteAdvice),
            (r"(?:cung|duong|leo).*(?:co\s+kho\s+khong|co\s+kho\s+ko|the\s+nao|di\s+may\s+ngay)", LeadIntent::RouteAdvice),
            (r"(?:di\s+toi\s+diem\s+nao|di\s+cung\s+nao|di\s+dau\s+dep)", LeadIntent::RouteAdvice),
            (r"co\s+the\s+ket\s+hop\s+di", LeadIntent::RouteAdvice),
            (r"co\s+bac\s+nao\s+di\s+cung.*chua", LeadIntent::RouteAdvice),
        ];

        let high_intent_patterns = high_intent_tuples
            .into_iter()
            .map(|(p, intent)| (Regex::new(&format!(r"(?i){}", p)).unwrap(), intent))
            .collect();

        let locations = vec![
            ("Fansipan", "fansipan"),
            ("Fansipan", "phan xi pang"),
            ("Putaleng", "putaleng"),
            ("Putaleng", "pu ta leng"),
            ("Pusilung", "pusilung"),
            ("Pusilung", "pu si lung"),
            ("Nam Kang Ho Tao", "nam kang ho tao"),
            ("Nam Kang Ho Tao", "namkang"),
            ("Nam Kang Ho Tao", "nam kang"),
            ("Bạch Mộc Lương Tử", "bach moc luong tu"),
            ("Bạch Mộc Lương Tử", "bach moc"),
            ("Kỳ Quan San", "ky quan san"),
            ("Kỳ Quan San", "kqs"),
            ("Lảo Thẩn", "lao than"),
            ("Lảo Thẩn", "lao tham"),
            ("Nhìu Cồ San", "nhiu co san"),
            ("Nhìu Cồ Ván", "nhiu co van"),
            ("Tà Chì Nhù", "ta chi nhu"),
            ("Tà Chì Nhù", "mo chi"),
            ("Tà Xùa", "ta xua"),
            ("Ngũ Chỉ Sơn", "ngu chi son"),
            ("Ngũ Chỉ Sơn", "ncs"),
            ("Tây Côn Lĩnh", "tay con linh"),
            ("Tây Côn Lĩnh", "tcl"),
            ("Lùng Cúng", "lung cung"),
            ("Pờ Ma Lung", "po ma lung"),
            ("Chu Va 12", "chu va 12"),
            ("Chu Va 12", "chuva 12"),
            ("Chu Va 12", "chuva"),
            ("Cán Chua Thìa Sảng", "can chua thia sang"),
            ("Cán Chua Thìa Sảng", "can chua"),
            ("Miêu Thạch Sơn", "mieu thach son"),
            ("Can Chu Miêu", "can chu mieu"),
            ("Núi Muối", "nui muoi"),
            ("Phu Sa Phìn", "phu sa phin"),
            ("Phu Sa Phìn", "phun sa phin"),
            ("Phu Sa Phìn", "phù sa phìn"),
            ("Samu", "samu"),
            ("Samu", "u bo"),
            ("Samu", "samu u bo"),
            ("Tả Liên Sơn", "ta lien son"),
            ("Hàm Lợn", "ham lon"),
            ("Cửa Tử", "cua tu"),
            ("Na Hang", "na hang"),
            ("Thác Nặm Me", "thac nam me"),
            ("Vườn quốc gia Ba Vì", "ba vi"),
            ("Vườn quốc gia Ba Vì", "vuon quoc gia ba vi"),
            ("Đà Giang", "da giang"),
            ("Hồ Mắt Ngọc", "ho mat ngoc"),
            ("Vân Sam", "van sam"),
            ("Chiêu Lầu Thi", "chieu lau thi"),
            ("Yên Tử", "yen tu"),
            ("Núi Bà Đen", "ba den"),
            ("Tà Năng", "ta nang"),
            ("Tà Năng", "ta nang phan dung"),
            ("Tà Năng", "phan dung"),
            ("Langbiang", "langbiang"),
            ("Langbiang", "lang biang"),
            ("Bidoup", "bidoup"),
            ("Lomburr", "lomburr"),
            ("Lomburr", "lom burr"),
            ("Tú Lệ", "tu le"),
            ("Cát Bà", "cat ba"),
            ("Tà Đùng", "ta dung"),
            ("K50", "k50"),
            ("Hang Én", "hang en"),
            ("Chư Yang Sin", "chu yang sin"),
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
            Regex::new(r"(?i)\b(?:ko|khong|k|khum|hong|a)\b\??").unwrap(),
            Regex::new(r"(?i)\b(?:hoi|xin|tu\s+van|muon|mun|co\s+ai|ai\s+co|ai\s+biet|bac\s+nao|co\s+doan\s+nao)\b").unwrap(),
        ];

        Self {
            seller_patterns,
            sharing_patterns,
            high_intent_patterns,
            locations,
            date_patterns,
            question_patterns,
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

        // 6. Kiểm tra Heuristic: Bài viết có nhắc địa danh + tín hiệu thắc mắc/hỏi han
        if has_trekking_context && has_question {
            // Suy đoán ý định theo từ khóa ngữ nghĩa
            let heuristic_intent = if normalized.contains("porter") || normalized.contains("poter") || normalized.contains("lan nghi") {
                LeadIntent::HiringPorter
            } else if normalized.contains("guide") || normalized.contains("huong dan vien") || normalized.contains("dan duong") {
                LeadIntent::HiringGuide
            } else if normalized.contains("tour") || normalized.contains("slot") || normalized.contains("ve") || normalized.contains("gia") {
                LeadIntent::LookingForTour
            } else if normalized.contains("ban") || normalized.contains("doan") || normalized.contains("nhom") || normalized.contains("team") || normalized.contains("ghep") || normalized.contains("dong doi") {
                LeadIntent::FindingCompanions
            } else {
                LeadIntent::RouteAdvice
            };

            return RuleResult::Positive {
                intent: heuristic_intent,
                confidence: 0.85,
                reason: "Phát hiện câu hỏi thắc mắc liên quan hành trình trekking".to_string(),
                location: detected_location,
                requested_date,
                summary: summary_snippet,
            };
        }

        // 7. Nếu có nhắc đến địa danh nhưng ý định chưa rõ ràng -> Chuyển tầng 2 (AI Gateway)
        RuleResult::Uncertain {
            location: detected_location,
            requested_date,
            reason: "Có đề cập đến trekking nhưng ý định chưa rõ ràng, cần AI thẩm định".to_string(),
        }
    }
}
