# Social Listener (100% Pure Rust Native Engine)

Hệ thống giám sát Facebook Groups & tự động phát hiện khách hàng tiềm năng (Lead Generation) cho dịch vụ Trekking & Leo núi Việt Nam.

## 🚀 Đặc Điểm Kỹ Thuật

- **100% Thuần Rust (Rust 2021 Edition / 1.80+)**: Single binary, loại bỏ hoàn toàn Python runtime, Node.js và VNC/Xvfb.
- **Clean Architecture & SOLID**: Phân tách nghiêm ngặt 4 tầng (`domain`, `application`, `infrastructure`, `presentation`).
- **Facebook Native Integration**: Tích hợp trực tiếp với `facebook-cli` (`bin/fb`) điều khiển headless Chrome qua Chrome DevTools Protocol (CDP).
- **Bộ phân loại Lead 2 tầng**:
  - *Tầng 1 (Rule Taxonomy)*: Chuẩn hóa Unicode NFC, khử dấu tiếng Việt NFD, tra cứu 50+ địa danh trekking VN, loại trừ seller/review với độ trễ < 0.1ms.
  - *Tầng 2 (Gemini 1.5 Flash AI)*: Thẩm định nhu cầu khi gặp các ca chưa rõ ràng, tích hợp Prompt Injection Guard.
- **Transactional Outbox & Idempotency**: Đảm bảo gửi tin cậy sang CRM Tổ Kiến (`anthill_be`) với HTTP header `Idempotency-Key` và cơ chế Exponential Backoff.
- **Tiêu thụ tài nguyên tối thiểu**: RAM runtime < 60MB, Docker image < 120MB.

---

## 📁 Cấu Trúc Mã Nguồn (Clean Architecture)

```
social_notification/facebook/
├── Cargo.toml
├── migrations/                  # 8 bảng CSDL Cốt lõi
│   └── 20261030000001_initial_schema.sql
├── bin/
│   └── fb                       # facebook-cli binary (CDP controller)
├── js/                          # Bộ script JS inject bóc tách DOM (feed.js, lib.js...)
├── src/
│   ├── domain/                  # Entities & Repository Traits
│   │   ├── post.rs
│   │   ├── lead.rs
│   │   ├── target.rs
│   │   ├── session.rs
│   │   ├── outbox.rs
│   │   └── repositories.rs
│   ├── application/             # Use Cases & Classifier
│   │   ├── classifier/          # Rule Taxonomy + Gemini AI Gateway
│   │   ├── use_cases/           # Crawl, Classify, Dispatch, Session
│   │   └── interfaces.rs
│   ├── infrastructure/          # Adapters, SQLx Pool, Reqwest Clients
│   │   ├── config.rs
│   │   ├── database/sqlite.rs
│   │   ├── cdp/engine.rs
│   │   └── external/anthill.rs
│   ├── presentation/            # Axum REST API & Background Tokio Workers
│   │   ├── api/                 # Endpoints REST & Prometheus Metrics
│   │   └── workers/             # Scheduler, Classifier, Outbox Notifier
│   └── main.rs
├── Dockerfile
├── docker-compose.yml
└── .env.example
```

---

## 🛠️ Hướng Dẫn Vận Hành

### 1. Chạy Cục Bộ (Cargo)

```bash
# Thiết lập cấu hình môi trường
cp .env.example .env

# Chạy server và các background workers
cargo run --release
```

Server sẽ lắng nghe tại: `http://localhost:8000`

### 2. Triển Khai Docker

```bash
docker compose up -d --build
```

### 3. Đồng Bộ Cookie Đăng Nhập (1-Click Cookie Sync)

Sử dụng tiện ích **Cookie-Editor** trên Chrome cá nhân (đã đăng nhập nick Facebook bot và tham gia các nhóm trekking):
1. Bấm Export -> Export as JSON.
2. Gửi request nạp cookie vào Social Listener:

```bash
curl -X POST http://localhost:8000/api/session/import \
  -H "Content-Type: application/json" \
  -d '{
    "session_id": "primary",
    "cookies": "[{\"name\":\"c_user\",\"value\":\"...\"},{\"name\":\"xs\",\"value\":\"...\"}]"
  }'
```

### 4. Danh Sách REST API

| Method | Endpoint | Mô Tả |
|---|---|---|
| `GET` | `/api/targets` | Danh sách nhóm đang theo dõi |
| `POST` | `/api/targets` | Thêm nhóm Facebook cần giám sát |
| `PUT` | `/api/targets/:id/toggle` | Bật/tắt quét một nhóm |
| `POST` | `/api/session/import` | Nạp cookie JSON mới |
| `GET` | `/api/posts` | Danh sách bài viết đã cào |
| `GET` | `/api/leads` | Danh sách khách hàng tiềm năng phát hiện được |
| `GET` | `/api/outbox` | Danh sách sự kiện Webhook |
| `POST` | `/api/outbox/:id/retry` | Thử lại sự kiện Dead-letter |
| `GET` | `/health/liveness` | Kubernetes liveness probe |
| `GET` | `/health/readiness` | Kubernetes readiness probe (kiểm tra CSDL) |
| `GET` | `/metrics` | Prometheus Metrics |
