import asyncio
import json
import os
import sys
import time
import urllib.request
import websockets

def get_cdp_ws_url():
    cdp_port = os.getenv("FB_CDP", "9222").strip()
    if cdp_port.startswith("ws://") or cdp_port.startswith("wss://"):
        return cdp_port
    port = cdp_port.lstrip(":")
    for attempt in range(1, 4):
        try:
            req = urllib.request.Request(
                f"http://127.0.0.1:{port}/json/version",
                headers={"Host": f"127.0.0.1:{port}"}
            )
            with urllib.request.urlopen(req, timeout=10) as resp:
                info = json.loads(resp.read().decode())
                ws = info.get("webSocketDebuggerUrl")
                if ws:
                    return ws
        except Exception as e:
            if attempt == 3:
                print(f"[inject_cookies] Failed to query Chrome on port {port}: {e}")
            time.sleep(1)
    return None

def resolve_cookie_file(arg_path=None):
    if arg_path and os.path.exists(arg_path):
        return arg_path
    candidates = [
        "/app/runtime/profile/primary_cookies.json",
        "./runtime/profile/primary_cookies.json",
        os.path.join(os.path.dirname(__file__), "..", "runtime", "profile", "primary_cookies.json"),
    ]
    for c in candidates:
        if os.path.exists(c):
            return os.path.abspath(c)
    return candidates[0]

def parse_cookie_list(raw_data):
    cookie_list = []
    if isinstance(raw_data, list):
        cookie_list = raw_data
    elif isinstance(raw_data, dict) and "cookies" in raw_data:
        if isinstance(raw_data["cookies"], list):
            cookie_list = raw_data["cookies"]
        elif isinstance(raw_data["cookies"], str):
            try:
                cookie_list = json.loads(raw_data["cookies"])
            except Exception:
                pass
    elif isinstance(raw_data, dict):
        for k, v in raw_data.items():
            cookie_list.append({"name": k, "value": str(v), "domain": ".facebook.com", "path": "/"})
    return cookie_list

def format_cookies_for_cdp(cookie_list):
    now = time.time()
    formatted = []
    core_fb_cookies = {"c_user", "xs", "fr", "datr", "sb", "ps_n", "ps_l"}

    for c in cookie_list:
        name = c.get("name")
        value = c.get("value")
        if not name or value is None:
            continue

        domain = c.get("domain") or ".facebook.com"
        if not domain.startswith(".") and not domain.startswith("http"):
            domain = "." + domain
        path = c.get("path") or "/"

        # Cookie-Editor & Chrome spec mapping:
        raw_same_site = str(c.get("sameSite", "")).lower().strip()
        if raw_same_site in ("no_restriction", "none"):
            same_site = "None"
            secure = True
        elif raw_same_site == "strict":
            same_site = "Strict"
            secure = bool(c.get("secure", False))
        elif raw_same_site == "lax":
            same_site = "Lax"
            secure = bool(c.get("secure", False))
        else:
            same_site = "None" if c.get("secure", True) else "Lax"
            secure = bool(c.get("secure", True))

        # Core Facebook auth cookies MUST be SameSite=None and Secure=True
        if name in core_fb_cookies:
            same_site = "None"
            secure = True

        http_only = bool(c.get("httpOnly", c.get("http_only", name in ("xs", "datr", "fr", "sb"))))

        expires_raw = c.get("expires") or c.get("expirationDate")
        try:
            expires = float(expires_raw) if expires_raw and float(expires_raw) > now else now + (180 * 86400)
        except (ValueError, TypeError):
            expires = now + (180 * 86400)

        formatted.append({
            "name": name,
            "value": str(value),
            "domain": domain,
            "path": path,
            "secure": secure,
            "httpOnly": http_only,
            "expires": expires,
            "sameSite": same_site
        })
    return formatted

async def inject_cookies_to_chrome(cookie_file):
    if not os.path.exists(cookie_file):
        print(f"[inject_cookies] Cookie file {cookie_file} does not exist, skipping.")
        return False

    try:
        with open(cookie_file, "r", encoding="utf-8") as f:
            raw_data = json.load(f)
    except Exception as e:
        print(f"[inject_cookies] Error reading {cookie_file}: {e}")
        return False

    raw_list = parse_cookie_list(raw_data)
    if not raw_list:
        print("[inject_cookies] No cookies found to inject.")
        return False

    formatted = format_cookies_for_cdp(raw_list)
    ws_url = get_cdp_ws_url()
    if not ws_url:
        print("[inject_cookies] Could not resolve Chrome CDP WebSocket URL")
        return False

    print(f"[inject_cookies] Injecting {len(formatted)} cookies into Chrome via CDP ({ws_url})...")
    async with websockets.connect(ws_url) as ws:
        req = {
            "id": 1,
            "method": "Storage.setCookies",
            "params": {"cookies": formatted}
        }
        await ws.send(json.dumps(req))
        resp = json.loads(await ws.recv())
        if "error" in resp:
            print(f"[inject_cookies] Storage.setCookies returned error: {resp['error']}")
            return False

        # Verify injection
        await ws.send(json.dumps({"id": 2, "method": "Storage.getCookies"}))
        get_res = json.loads(await ws.recv())
        chrome_cookies = get_res.get("result", {}).get("cookies", [])
        names = {c.get("name") for c in chrome_cookies}
        has_c_user = "c_user" in names
        has_xs = "xs" in names
        print(f"[inject_cookies] Injection complete. Chrome now has {len(chrome_cookies)} cookies. c_user={has_c_user}, xs={has_xs}")
        return has_c_user and has_xs

async def sync_cookies_from_chrome(cookie_file):
    ws_url = get_cdp_ws_url()
    if not ws_url:
        return False

    try:
        async with websockets.connect(ws_url) as ws:
            await ws.send(json.dumps({"id": 1, "method": "Storage.getCookies"}))
            res = json.loads(await ws.recv())
            chrome_cookies = res.get("result", {}).get("cookies", [])

        # Only sync if c_user and xs exist in Chrome (prevent writing logged-out state)
        c_user = next((c for c in chrome_cookies if c.get("name") == "c_user" and c.get("value")), None)
        xs = next((c for c in chrome_cookies if c.get("name") == "xs" and c.get("value")), None)

        if not c_user or not xs:
            return False

        os.makedirs(os.path.dirname(os.path.abspath(cookie_file)), exist_ok=True)
        # Format cookies cleanly for persistence
        persisted_list = []
        for c in chrome_cookies:
            persisted_list.append({
                "name": c.get("name"),
                "value": c.get("value"),
                "domain": c.get("domain", ".facebook.com"),
                "path": c.get("path", "/"),
                "secure": c.get("secure", True),
                "httpOnly": c.get("httpOnly", False),
                "expirationDate": c.get("expires"),
                "sameSite": "no_restriction" if str(c.get("sameSite", "")).lower() == "none" else c.get("sameSite", "lax").lower()
            })

        tmp_file = cookie_file + ".tmp"
        with open(tmp_file, "w", encoding="utf-8") as f:
            json.dump(persisted_list, f, indent=2, ensure_ascii=False)
        os.replace(tmp_file, cookie_file)
        print(f"[inject_cookies] Successfully synced {len(persisted_list)} fresh cookies from Chrome to {cookie_file}")
        return True
    except Exception as e:
        print(f"[inject_cookies] Failed to sync cookies from Chrome: {e}")
        return False

async def ensure_active_session(cookie_file):
    ws_url = get_cdp_ws_url()
    if not ws_url:
        return False

    try:
        async with websockets.connect(ws_url) as ws:
            await ws.send(json.dumps({"id": 1, "method": "Storage.getCookies"}))
            res = json.loads(await ws.recv())
            chrome_cookies = res.get("result", {}).get("cookies", [])

        has_c_user = any(c.get("name") == "c_user" and c.get("value") for c in chrome_cookies)
        has_xs = any(c.get("name") == "xs" and c.get("value") for c in chrome_cookies)

        if has_c_user and has_xs:
            # Active! Sync any updated tokens (fr, etc.) back to file
            await sync_cookies_from_chrome(cookie_file)
            return True

        # Chrome lost session cookies! Try re-injecting from persistent primary_cookies.json
        print("[inject_cookies] Session cookies missing in Chrome. Attempting auto-recovery from file...")
        success = await inject_cookies_to_chrome(cookie_file)
        return success
    except Exception as e:
        print(f"[inject_cookies] ensure_active_session error: {e}")
        return False

async def send_heartbeat():
    cdp_port = os.getenv("FB_CDP", "9222").strip().lstrip(":")
    client = urllib.request
    try:
        with client.urlopen(f"http://127.0.0.1:{cdp_port}/json/list", timeout=5) as r:
            pages = json.loads(r.read().decode())
    except Exception as e:
        print(f"[heartbeat] Could not list Chrome tabs: {e}")
        return False

    fb_page = next((p for p in pages if "facebook.com" in p.get("url", "") and p.get("type") == "page"), None)

    if not fb_page:
        # Create Facebook anchor tab if not present
        try:
            req = urllib.request.Request(f"http://127.0.0.1:{cdp_port}/json/new?https://www.facebook.com/", method="PUT")
            with client.urlopen(req, timeout=5) as r:
                fb_page = json.loads(r.read().decode())
                print(f"[heartbeat] Opened persistent Facebook anchor tab: {fb_page.get('id')}")
        except Exception as e:
            print(f"[heartbeat] Failed to open Facebook anchor tab: {e}")
            return False

    # Dispatch a lightweight presence event in the anchor tab
    ws_url = fb_page.get("webSocketDebuggerUrl")
    if ws_url:
        try:
            async with websockets.connect(ws_url) as ws:
                # Trigger document visibility / focus to keep Comet presence active
                expr = "document.dispatchEvent(new Event('visibilitychange')); window.dispatchEvent(new Event('focus')); document.title;"
                await ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate", "params": {"expression": expr}}))
                res = json.loads(await asyncio.wait_for(ws.recv(), timeout=3.0))
                title = res.get("result", {}).get("result", {}).get("value", "")
                print(f"[heartbeat] Presence heartbeat pinged. Anchor tab title: {title[:40]}")
                return True
        except Exception as e:
            print(f"[heartbeat] Failed to dispatch presence in anchor tab: {e}")
            return False
    return False

async def main():
    args = sys.argv[1:]
    cookie_file = resolve_cookie_file(args[0] if args and not args[0].startswith("--") else None)

    if "--sync-back" in args:
        success = await sync_cookies_from_chrome(cookie_file)
        sys.exit(0 if success else 1)
    elif "--ensure-active" in args:
        success = await ensure_active_session(cookie_file)
        sys.exit(0 if success else 1)
    elif "--heartbeat" in args:
        active = await ensure_active_session(cookie_file)
        if active:
            await send_heartbeat()
        sys.exit(0 if active else 1)
    else:
        success = await inject_cookies_to_chrome(cookie_file)
        sys.exit(0 if success else 1)

if __name__ == "__main__":
    asyncio.run(main())
