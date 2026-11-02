#!/usr/bin/env bash
set -e

# Khởi động Chrome lần đầu bằng recycle script
/app/bin/recycle_chrome.sh

# Cấu hình timeout mặc định 300s (5 phút) cho các tác vụ facebook-cli
export FB_TIMEOUT="${FB_TIMEOUT:-300}"
export FB_CDP="9222"
echo "Configured FB_TIMEOUT=$FB_TIMEOUT seconds, FB_CDP=9222."

echo "Starting Social Listener service..."
exec "$@"
