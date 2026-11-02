#!/usr/bin/env bash
set -e

echo "[Chrome-Recycle] Starting Chrome recycle procedure..."

# 1. Kill any existing chrome processes (chỉ nhắm đúng binary chrome, không kill chính script này)
pkill -9 -f "/opt/google/chrome" || true
sleep 1

# 2. Clean temporary disk cache and socket files to recover memory and disk space
rm -rf /dev/shm/chrome-cache/* /tmp/chrome.log /root/.cache/fb/fb.sock /app/runtime/profile/fb.sock || true
mkdir -p /dev/shm/chrome-cache /root/.cache/fb/chrome

# 3. Launch Google Chrome with low-footprint performance flags
echo "[Chrome-Recycle] Launching new Chrome instance with minimal resource flags..."
/opt/google/chrome/chrome \
    --headless=new \
    --no-sandbox \
    --disable-dev-shm-usage \
    --disable-gpu \
    --disable-software-rasterizer \
    --disable-webgl \
    --disable-3d-apis \
    --disable-accelerated-2d-canvas \
    --disable-gpu-rasterization \
    --blink-settings=imagesEnabled=false \
    --disk-cache-dir=/dev/shm/chrome-cache \
    --disk-cache-size=10485760 \
    --media-cache-size=1 \
    --disable-component-update \
    --disable-domain-reliability \
    --disable-features=OptimizationGuideModelDownloading,OptimizationHints,MediaRouter,Translate,WebUIOmniboxPopup,CalculateNativeWinOcclusion,AudioServiceOutOfProcess,OnDeviceModel \
    --disable-background-timer-throttling \
    --disable-renderer-backgrounding \
    --remote-debugging-port=9222 \
    --remote-debugging-address=0.0.0.0 \
    --remote-allow-origins=* \
    --renderer-process-limit=1 \
    --disable-background-networking \
    --disable-sync \
    --disable-default-apps \
    --disable-extensions \
    --mute-audio \
    --disable-breakpad \
    --disable-crash-reporter \
    --js-flags="--max-old-space-size=256" \
    --no-default-browser-check \
    --no-first-run \
    --user-data-dir=/root/.cache/fb/chrome \
    about:blank >/tmp/chrome.log 2>&1 &

CHROME_PID=$!

echo "[Chrome-Recycle] Waiting for Chrome CDP port 9222 to become available..."
for i in {1..30}; do
    if curl -s -H "Host: 127.0.0.1:9222" http://127.0.0.1:9222/json/version >/dev/null 2>&1; then
        echo "[Chrome-Recycle] Chrome CDP is ready on port 9222 (PID: $CHROME_PID)."
        break
    fi
    sleep 0.5
done

# 4. Re-inject primary cookies if available
if [ -f /app/runtime/profile/primary_cookies.json ]; then
    echo "[Chrome-Recycle] Re-injecting cookies from /app/runtime/profile/primary_cookies.json..."
    python3 /app/bin/inject_cookies.py || true
fi

echo "[Chrome-Recycle] Chrome recycle completed successfully."
