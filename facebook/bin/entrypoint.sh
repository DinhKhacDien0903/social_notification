#!/usr/bin/env bash
set -e

# Đảm bảo quyền thực thi cho các file trong /app/bin
chmod +x /app/bin/* 2>/dev/null || true

# Khởi động Chrome lần đầu bằng recycle script
/app/bin/recycle_chrome.sh

# Cấu hình timeout mặc định 60s cho các tác vụ facebook-cli (Fail-Fast chống treo kết nối)
export FB_TIMEOUT="${FB_TIMEOUT:-60}"
export FB_CDP="9222"
echo "Configured FB_TIMEOUT=$FB_TIMEOUT seconds, FB_CDP=9222."

echo "Starting Social Listener service..."
exec "$@"
