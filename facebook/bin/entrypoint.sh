#!/usr/bin/env bash
set -e

# Dọn dẹp socket cũ nếu có
rm -f /root/.cache/fb/fb.sock /app/runtime/profile/fb.sock /tmp/chrome.log || true

echo "Starting Headless Chrome on CDP port 9222 with performance flags..."
/opt/google/chrome/chrome \
    --headless=new \
    --no-sandbox \
    --disable-dev-shm-usage \
    --disable-gpu \
    --remote-debugging-port=9222 \
    --remote-debugging-address=0.0.0.0 \
    --remote-allow-origins=* \
    --renderer-process-limit=2 \
    --disable-background-networking \
    --disable-sync \
    --disable-default-apps \
    --disable-extensions \
    --mute-audio \
    --no-default-browser-check \
    --no-first-run \
    --user-data-dir=/root/.cache/fb/chrome \
    about:blank >/tmp/chrome.log 2>&1 &

CHROME_PID=$!

echo "Waiting for Chrome CDP port 9222 to become available..."
for i in {1..20}; do
    if curl -s -H "Host: 127.0.0.1:9222" http://127.0.0.1:9222/json/version >/dev/null 2>&1; then
        echo "Chrome CDP ready on port 9222."
        break
    fi
    sleep 0.5
done

# Tự động trích xuất WebSocket Debugger URL của Chrome
WS_URL=$(curl -s -H "Host: 127.0.0.1:9222" http://127.0.0.1:9222/json/version | python3 -c "import sys, json; print(json.load(sys.stdin).get('webSocketDebuggerUrl', ''))" 2>/dev/null || true)
if [ -n "$WS_URL" ]; then
    export FB_CDP="$WS_URL"
    echo "Resolved Chrome CDP WebSocket URL: $FB_CDP"
fi

if [ -f /app/runtime/profile/primary_cookies.json ]; then
    echo "Restoring cookies from /app/runtime/profile/primary_cookies.json..."
    python3 /app/bin/inject_cookies.py || true
fi

echo "Starting Social Listener service..."
exec "$@"
