// Unit tests cho Quy tắc Nhận diện lỗi Auth & Circuit Breaker (Ngắt crawl tại lần 2 & Alert Admin)

#[cfg(test)]
mod tests {
    use chrono::Utc;

    // Mô phỏng logic is_auth_error
    fn is_auth_error(err_str: &str) -> bool {
        let lower = err_str.to_lowercase();
        lower.contains("not logged in")
            || lower.contains("login_required")
            || lower.contains("checkpoint")
            || lower.contains("fb login")
            || lower.contains("login.php")
            || lower.contains("/login")
            || lower.contains("session expired")
            || lower.contains("invalid session")
            || lower.contains("session not initialized")
            || (lower.contains("cookie") && (lower.contains("missing") || lower.contains("invalid") || lower.contains("expired") || lower.contains("error")))
            || lower.contains("c_user")
            || lower.contains("unauthorized")
            || lower.contains("401 unauthorized")
            || lower.contains("403 forbidden")
    }

    struct MockSession {
        pub status: String,
        pub auth_failure_count: i32,
    }

    impl MockSession {
        pub fn is_ready(&self) -> bool {
            self.status == "READY" && self.auth_failure_count < 2
        }
    }

    #[test]
    fn test_is_auth_error_detection() {
        // Positive auth error cases
        assert!(is_auth_error("error: not logged in to Facebook: run `fb login` (it reuses the session of your normal Chrome; one Allow click)"));
        assert!(is_auth_error("LOGIN_REQUIRED: Target page requested login"));
        assert!(is_auth_error("Facebook redirected to https://www.facebook.com/checkpoint/150000000/"));
        assert!(is_auth_error("Session expired while scrolling feed"));
        assert!(is_auth_error("Cookie missing c_user parameter"));
        assert!(is_auth_error("HTTP 401 Unauthorized"));
        assert!(is_auth_error("HTTP 403 Forbidden"));
        assert!(is_auth_error("Invalid session credentials provided"));

        // Negative non-auth cases (Transient / Network errors that should NOT trigger auth circuit breaker)
        assert!(!is_auth_error("Operation timed out after 300 seconds"));
        assert!(!is_auth_error("Connection refused to Chrome CDP 127.0.0.1:9222"));
        assert!(!is_auth_error("Group 123456 returned 404 Not Found"));
        assert!(!is_auth_error("DOM parsing syntax error: Unexpected EOF"));
    }

    #[test]
    fn test_circuit_breaker_threshold_at_two_attempts() {
        let mut session = MockSession {
            status: "READY".to_string(),
            auth_failure_count: 0,
        };

        // Lần 0: Phiên bình thường -> Cho phép crawl
        assert!(session.is_ready());

        // Lần 1: Lỗi Auth lần đầu (Group A) -> Tăng bộ đếm lên 1, vẫn cho phép crawl thêm 1 lần (attempt 2)
        session.auth_failure_count += 1;
        assert_eq!(session.auth_failure_count, 1);
        assert!(session.is_ready(), "Tại lần 1 vẫn cho phép retry attempt 2");

        // Lần 2: Lỗi Auth lần hai (Group B hoặc cùng Group A) -> Bộ đếm lên 2
        session.auth_failure_count += 1;
        assert_eq!(session.auth_failure_count, 2);

        // KHI ĐẠT LẦN 2: Ngắt phiên, chuyển sang LOGIN_REQUIRED
        assert!(!session.is_ready(), "Tại lần 2: Ngắt hoàn toàn (Circuit Breaker)");
        session.status = "LOGIN_REQUIRED".to_string();
        assert!(!session.is_ready());

        // KHI IMPORT SESSION THÀNH CÔNG: Reset về 0 và READY -> Cho phép crawl lại
        session.auth_failure_count = 0;
        session.status = "READY".to_string();
        assert!(session.is_ready(), "Sau khi nạp session hợp lệ, crawler được kích hoạt lại");
    }
}
