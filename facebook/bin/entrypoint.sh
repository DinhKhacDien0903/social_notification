#!/usr/bin/env bash
set -e

echo "Starting Headless Chrome on CDP port 9222..."
/opt/google/chrome/chrome \
    --headless=new \
    --no-sandbox \
    --disable-dev-shm-usage \
    --disable-gpu \
    --remote-debugging-port=9222 \
    --user-data-dir=/root/.cache/fb/chrome \
    about:blank >/tmp/chrome.log 2>&1 &

CHROME_PID=$!

echo "Waiting for Chrome CDP port 9222 to become available..."
for i in {1..20}; do
    if curl -s http://127.0.0.1:9222/json/version >/dev/null 2>&1; then
        echo "Chrome CDP ready on port 9222."
        break
    fi
    sleep 0.5
done

if [ -f /app/runtime/profile/primary_cookies.json ]; then
    echo "Restoring cookies from /app/runtime/profile/primary_cookies.json..."
    python3 /app/bin/inject_cookies.py || true
fi

echo "Starting Social Listener service..."
exec "$@"
