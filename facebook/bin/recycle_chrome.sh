#!/usr/bin/env bash
set -e

echo "[Chrome-Recycle] Starting Chrome recycle procedure..."

CHROME_PROFILE_DIR="/app/runtime/profile/chrome"

# 1. Gracefully terminate existing Chrome processes (SIGTERM first to flush SQLite/storage, fallback to SIGKILL)
if pgrep -f "/opt/google/chrome" >/dev/null 2>&1; then
    echo "[Chrome-Recycle] Sending SIGTERM to existing Chrome processes..."
    pkill -15 -f "/opt/google/chrome" || true
    for i in {1..15}; do
        if ! pgrep -f "/opt/google/chrome" >/dev/null 2>&1; then
            break
        fi
        sleep 0.2
    done
    if pgrep -f "/opt/google/chrome" >/dev/null 2>&1; then
        echo "[Chrome-Recycle] Chrome still running, forcing SIGKILL..."
        pkill -9 -f "/opt/google/chrome" || true
        sleep 0.5
    fi
fi

# 2. Clean temporary disk cache and lock files, PRESERVE persistent profile (cookies, localStorage, IndexedDB)
echo "[Chrome-Recycle] Cleaning temporary cache and lock files..."
rm -rf /dev/shm/chrome-cache/* /tmp/chrome.log /root/.cache/fb/fb.sock /app/runtime/profile/fb.sock || true
rm -f "$CHROME_PROFILE_DIR/Singleton"* "$CHROME_PROFILE_DIR/Default/Singleton"* "$CHROME_PROFILE_DIR/Default/LOCK" "$CHROME_PROFILE_DIR/Default/"*-journal 2>/dev/null || true
mkdir -p /dev/shm/chrome-cache "$CHROME_PROFILE_DIR"

# 3. Detect actual installed Chrome major version to prevent Client-Hints vs User-Agent mismatch
CHROME_VER=$(/opt/google/chrome/chrome --version 2>/dev/null | grep -oE '[0-9]+(\.[0-9]+)+' | head -1)
CHROME_MAJOR=$(echo "$CHROME_VER" | cut -d. -f1)
if [ -z "$CHROME_MAJOR" ]; then
    CHROME_MAJOR="156"
fi
USER_AGENT="Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${CHROME_MAJOR}.0.0.0 Safari/537.36"
echo "[Chrome-Recycle] Detected Chrome version: $CHROME_VER (Major: $CHROME_MAJOR), using UA: $USER_AGENT"

# 4. Configure optional proxy if FB_PROXY is set
PROXY_ARGS=()
if [ -n "$FB_PROXY" ]; then
    echo "[Chrome-Recycle] Using configured proxy: $FB_PROXY"
    PROXY_ARGS+=("--proxy-server=$FB_PROXY")
fi

# 5. Launch Google Chrome with stealth flags and persistent profile directory
# - No SwiftShader WebGL fingerprint
# - Images enabled to allow tracking beacons (/ajax/bz) and prevent headless scraper detection
# - Heavy video/audio streams blocked via host-resolver-rules and document-user-activation-required
echo "[Chrome-Recycle] Launching new Chrome instance with persistent profile: $CHROME_PROFILE_DIR..."
/opt/google/chrome/chrome \
    --headless=new \
    --no-sandbox \
    --disable-dev-shm-usage \
    --disable-gpu \
    --disk-cache-dir=/dev/shm/chrome-cache \
    --disk-cache-size=10485760 \
    --media-cache-size=1 \
    --disable-component-update \
    --disable-domain-reliability \
    --disable-features=OptimizationGuideModelDownloading,OptimizationHints,MediaRouter,Translate,WebUIOmniboxPopup,CalculateNativeWinOcclusion,AudioServiceOutOfProcess,OnDeviceModel,MediaStream,WebRtcHideLocalIpsWithMdns,PreloadMediaEngagementData \
    --disable-background-timer-throttling \
    --disable-renderer-backgrounding \
    --remote-debugging-port=9222 \
    --remote-debugging-address=0.0.0.0 \
    --remote-allow-origins=* \
    --renderer-process-limit=2 \
    --disable-background-networking \
    --disable-sync \
    --disable-default-apps \
    --disable-extensions \
    --mute-audio \
    --autoplay-policy=document-user-activation-required \
    --disable-speech-api \
    --disable-media-session-api \
    --host-resolver-rules="MAP *video*.fbcdn.net ~NOTFOUND, MAP *video*.facebook.com ~NOTFOUND" \
    --disable-breakpad \
    --disable-crash-reporter \
    --js-flags="--max-old-space-size=256" \
    --no-default-browser-check \
    --no-first-run \
    --disable-blink-features=AutomationControlled \
    --disable-infobars \
    --user-agent="$USER_AGENT" \
    --window-size=1366,900 \
    --lang=vi-VN,vi,en-US,en \
    --user-data-dir="$CHROME_PROFILE_DIR" \
    "${PROXY_ARGS[@]}" \
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

# 6. Re-inject primary cookies if available
if [ -f /app/runtime/profile/primary_cookies.json ]; then
    echo "[Chrome-Recycle] Re-injecting cookies from /app/runtime/profile/primary_cookies.json..."
    python3 /app/bin/inject_cookies.py || true
fi

# 7. Establish persistent Facebook Comet anchor tab to prevent idle session timeout
echo "[Chrome-Recycle] Initializing persistent Facebook Comet anchor tab..."
curl -s -X PUT -H "Host: 127.0.0.1:9222" "http://127.0.0.1:9222/json/new?https://www.facebook.com/" >/dev/null 2>&1 || true

echo "[Chrome-Recycle] Chrome recycle completed successfully."
