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
    },
    Negative {
        reason: String,
    },
    Uncertain {
        location: Option<String>,
        reason: String,
    },
}

pub struct RuleClassifier {
    seller_patterns: Vec<Regex>,
    sharing_patterns: Vec<Regex>,
    high_intent_patterns: Vec<(Regex, LeadIntent)>,
    locations: Vec<(&'static str, &'static str)>, // (canonical_display, search_keyword_normalized)
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
            r"gia chi tu",
            r"combo du lich",
            r"inbox de nhan bao gia",
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
            r"ban giay trekking",
            r"uu dai dac biet",
            r"gia uu dai",
            r"tro gia",
        ];

        let seller_patterns = seller_regexes
            .into_iter()
            .map(|p| Regex::new(&format!(r"(?i)\b{}\b", p)).unwrap())
            .collect();

        let sharing_regexes = vec![
            r"chia se hanh trinh",
            r"review chuyen di",
            r"trai nghiem cua minh",
            r"gui ca nha bo anh",
            r"anh dep chuyen di",
            r"nhat ky hanh trinh",
            r"ky niem chuyen di",
            r"mot so hinh anh",
            r"trai nghiem tuyet voi",
        ];

        let sharing_patterns = sharing_regexes
            .into_iter()
            .map(|p| Regex::new(&format!(r"(?i)\b{}\b", p)).unwrap())
            .collect();

        let high_intent_tuples = vec![
            (r"can tim tour", LeadIntent::LookingForTour),
            (r"tim tour", LeadIntent::LookingForTour),
            (r"co tour nao", LeadIntent::LookingForTour),
            (r"tu van tour", LeadIntent::LookingForTour),
            (r"xin review tour", LeadIntent::LookingForTour),
            (r"co ai ban tour", LeadIntent::LookingForTour),
            (r"tim porter", LeadIntent::HiringPorter),
            (r"thue porter", LeadIntent::HiringPorter),
            (r"can porter", LeadIntent::HiringPorter),
            (r"tim huong dan vien", LeadIntent::HiringGuide),
            (r"tim guide", LeadIntent::HiringGuide),
            (r"can guide", LeadIntent::HiringGuide),
            (r"thue guide", LeadIntent::HiringGuide),
            (r"can nguoi dan duong", LeadIntent::HiringGuide),
            (r"tim nhom ghep", LeadIntent::FindingCompanions),
            (r"ghep doan", LeadIntent::FindingCompanions),
            (r"tim ban dong hanh", LeadIntent::FindingCompanions),
            (r"co ai di chung", LeadIntent::FindingCompanions),
            (r"co ai leo .* khong", LeadIntent::FindingCompanions),
            (r"chi phi di .* het bao nhieu", LeadIntent::RouteAdvice),
            (r"xin kinh nghiem leo", LeadIntent::RouteAdvice),
            (r"xin lich trinh", LeadIntent::RouteAdvice),
            (r"tu van cung", LeadIntent::RouteAdvice),
        ];

        let high_intent_patterns = high_intent_tuples
            .into_iter()
            .map(|(p, intent)| (Regex::new(&format!(r"(?i)\b{}\b", p)).unwrap(), intent))
            .collect();

        let locations = vec![
            ("Fansipan", "fansipan"),
            ("Tà Năng - Phan Dũng", "ta nang phan dung"),
            ("Tà Năng", "ta nang"),
            ("Phan Dũng", "phan dung"),
            ("Putaleng", "putaleng"),
            ("Pusilung", "pusilung"),
            ("Nam Kang Ho Tao", "nam kang ho tao"),
            ("Bạch Mộc Lương Tử", "bach moc luong tu"),
            ("Kỳ Quan San", "ky quan san"),
            ("Lảo Thẩn", "lao than"),
            ("Nhìu Cồ Ván", "nhiu co van"),
            ("Tà Chì Nhù", "ta chi nhu"),
            ("Tà Xùa", "ta xua"),
            ("Ngũ Chỉ Sơn", "ngu chi son"),
            ("Chiêu Lầu Thi", "chieu lau thi"),
            ("Tây Côn Lĩnh", "tay con linh"),
            ("Lùng Cúng", "lung cung"),
            ("Pờ Ma Lung", "po ma lung"),
            ("Bidoup", "bidoup"),
            ("Núi Bà Đen", "nui ba den"),
            ("Chứa Chan", "chua chan"),
            ("Núi Dinh", "nui dinh"),
            ("Mã Đà", "ma da"),
            ("K50", "k50"),
            ("Hòn Bà", "hon ba"),
            ("Cực Đông", "cuc dong"),
            ("Pù Luông", "pu luong"),
            ("Hang Én", "hang en"),
            ("Sơn Đoòng", "son doong"),
            ("Tú Làn", "tu lan"),
            ("Pha Luông", "pha luong"),
            ("Đỉnh Gió", "dinh gio"),
            ("Sam Khỏe", "sam khoe"),
            ("Núi Chúa", "nui chua"),
            ("Bù Gia Mập", "bu gia map"),
            ("Cát Tiên", "cat tien"),
            ("Ba Bể", "ba be"),
            ("Hà Giang", "ha giang"),
            ("Y Tý", "y ty"),
            ("Mộc Châu", "moc chau"),
            ("Sapa", "sapa"),
            ("Đà Lạt", "da lat"),
        ];

        Self {
            seller_patterns,
            sharing_patterns,
            high_intent_patterns,
            locations,
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

        // Nếu không chứa bất kỳ địa danh trekking nào -> Loại bỏ bài viết ngoài lề
        if detected_location.is_none() && !normalized.contains("trekking") && !normalized.contains("leo nui") {
            return RuleResult::Negative {
                reason: "Không chứa địa danh trekking hoặc từ khóa liên quan leo núi".to_string(),
            };
        }

        // 3. Kiểm tra Bài chia sẻ/Review thuần túy
        for pat in &self.sharing_patterns {
            if pat.is_match(&normalized) {
                return RuleResult::Negative {
                    reason: "Khớp từ khóa chia sẻ hành trình/review chuyến đi".to_string(),
                };
            }
        }

        // 4. Kiểm tra Ý định mua cao (High Intent)
        for (pat, intent) in &self.high_intent_patterns {
            if pat.is_match(&normalized) {
                return RuleResult::Positive {
                    intent: intent.clone(),
                    confidence: 0.92,
                    reason: format!("Khớp mẫu ý định mua cao tầng 1: {:?}", intent),
                    location: detected_location,
                };
            }
        }

        // 5. Nếu có nhắc đến địa danh nhưng ý định chưa rõ ràng -> Chuyển tầng 2 (AI Gateway)
        RuleResult::Uncertain {
            location: detected_location,
            reason: "Có đề cập đến trekking nhưng ý định chưa rõ ràng, cần AI thẩm định".to_string(),
        }
    }
}
