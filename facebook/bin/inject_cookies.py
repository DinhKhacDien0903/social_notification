import asyncio
import json
import os
import time
import urllib.request
import websockets

async def main():
    cookie_file = "/app/runtime/profile/primary_cookies.json"
    if not os.path.exists(cookie_file):
        print(f"Cookie file {cookie_file} does not exist, skipping injection.")
        return

    try:
        with open(cookie_file, "r", encoding="utf-8") as f:
            raw_data = json.load(f)
    except Exception as e:
        print(f"Error reading {cookie_file}: {e}")
        return

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

    if not cookie_list:
        print("No cookies found to inject.")
        return

    now = time.time()
    formatted_cookies = []
    for c in cookie_list:
        name = c.get("name")
        value = c.get("value")
        if not name or value is None:
            continue
        domain = c.get("domain") or ".facebook.com"
        if not domain.startswith(".") and not domain.startswith("http"):
            domain = "." + domain
        path = c.get("path") or "/"
        secure = bool(c.get("secure", True))
        http_only = bool(c.get("httpOnly", c.get("http_only", name in ("xs", "datr", "fr", "sb"))))

        # Ensure cookies are persistent across Chrome restarts instead of temporary session cookies
        expires_raw = c.get("expires") or c.get("expirationDate")
        try:
            expires = float(expires_raw) if expires_raw and float(expires_raw) > now else now + (365 * 86400)
        except (ValueError, TypeError):
            expires = now + (365 * 86400)

        same_site = c.get("sameSite") or ("None" if secure else "Lax")
        if same_site not in ("Strict", "Lax", "None"):
            same_site = "Lax"

        formatted_cookies.append({
            "name": name,
            "value": str(value),
            "domain": domain,
            "path": path,
            "secure": secure,
            "httpOnly": http_only,
            "expires": expires,
            "sameSite": same_site
        })

    cdp_port = os.getenv("FB_CDP", "9222").strip()
    if cdp_port.startswith("ws://") or cdp_port.startswith("wss://"):
        ws_url = cdp_port
    else:
        port = cdp_port.lstrip(":")
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/json/version", timeout=5) as resp:
                info = json.loads(resp.read().decode())
                ws_url = info.get("webSocketDebuggerUrl")
        except Exception as e:
            print(f"Failed to query Chrome on port {port}: {e}")
            return

    if not ws_url:
        print("Could not find webSocketDebuggerUrl")
        return

    print(f"Injecting {len(formatted_cookies)} persistent cookies into Chrome via CDP ({ws_url})...")
    async with websockets.connect(ws_url) as ws:
        req = {
            "id": 1,
            "method": "Storage.setCookies",
            "params": {"cookies": formatted_cookies}
        }
        await ws.send(json.dumps(req))
        resp = await ws.recv()
        print(f"CDP Storage.setCookies response: {resp}")

if __name__ == "__main__":
    asyncio.run(main())
