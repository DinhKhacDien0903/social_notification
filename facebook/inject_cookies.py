import asyncio
import json
import urllib.request
import websockets

async def main():
    try:
        with urllib.request.urlopen("http://127.0.0.1:9222/json/version") as resp:
            info = json.loads(resp.read().decode())
    except Exception as e:
        print("Failed to get Chrome debug info:", e)
        return

    ws_url = info.get("webSocketDebuggerUrl")
    print("Connecting to CDP ws:", ws_url)
    
    cookies = [
        {"name": "c_user", "value": "61594533200183", "domain": ".facebook.com", "path": "/", "secure": True, "httpOnly": False},
        {"name": "xs", "value": "41%3AziGMj3v1LOnJsw%3A2%3A1793553454%3A-1%3A-1%3A%3AAc06CgpknyqKK2qMXsaSSYh0MKAW2NphBM3kF6xjFA", "domain": ".facebook.com", "path": "/", "secure": True, "httpOnly": True},
        {"name": "datr", "value": "4kZeaoK9e370AXaUe17FS_Mq", "domain": ".facebook.com", "path": "/", "secure": True, "httpOnly": True},
        {"name": "fr", "value": "1hQz3GZ8CF1n3WF6W.AWfCBaPvI1xIOG5oLCfy6x9OhWbkDCyc6_G1NF9Kzr-sRjqhBaA.Bq53Qx..AAA.0.0.Bq53Qx.AWf1MN6omi0287tqvkqSITkjbp8", "domain": ".facebook.com", "path": "/", "secure": True, "httpOnly": True},
        {"name": "sb", "value": "4kZeajRrs7f-irgmnIrYHTur", "domain": ".facebook.com", "path": "/", "secure": True, "httpOnly": True},
        {"name": "locale", "value": "vi_VN", "domain": ".facebook.com", "path": "/", "secure": True, "httpOnly": False},
        {"name": "ps_l", "value": "1", "domain": ".facebook.com", "path": "/", "secure": True, "httpOnly": True},
        {"name": "ps_n", "value": "1", "domain": ".facebook.com", "path": "/", "secure": True, "httpOnly": True},
        {"name": "wd", "value": "1920x945", "domain": ".facebook.com", "path": "/", "secure": True, "httpOnly": False}
    ]
    
    async with websockets.connect(ws_url) as ws:
        req = {"id": 1, "method": "Storage.setCookies", "params": {"cookies": cookies}}
        await ws.send(json.dumps(req))
        res = await ws.recv()
        print("Storage.setCookies response:", res)

if __name__ == "__main__":
    asyncio.run(main())
