// Unit tests cho Tầng 1: Rule Taxonomy
#[cfg(test)]
mod tests {
    use unicode_normalization::UnicodeNormalization;

    fn strip_accents(s: &str) -> String {
        s.nfd()
            .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
            .collect::<String>()
            .to_lowercase()
            .replace('đ', "d")
            .replace('Đ', "d")
    }

    #[test]
    fn test_strip_accents() {
        let input = "Hội Đam Mê Trekking & Leo Núi Fansipan Tà Năng Phan Dũng";
        let output = strip_accents(input);
        assert!(output.contains("fansipan"));
        assert!(output.contains("ta nang phan dung"));
        assert!(output.contains("leo nui"));
    }
}
