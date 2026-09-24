---
title: Browser Extensions
description: How to intercept downloads in Chrome, Edge, and Firefox using limedl
---

# Browser Extensions

Thanks to limedl's native Aria2 RPC server, you can connect existing popular browser extensions to seamlessly route web downloads into limedl.

## Recommended Extensions

1. **Aria2 Explorer** (Recommended — feature-rich with smart intercept filters)
2. **Camtd** (Clean, minimalist Aria2 web interceptor)
3. **Tampermonkey User Scripts** (For cloud drives and custom download sites)

## Setup in 3 Simple Steps

### Step 1: Ensure RPC is Active
In limedl **Settings -> Aria2 RPC**:
- **Enable Aria2 RPC**: Checked
- **Port**: Default is `6800`
- **Secret Token**: Optional (leave empty or set a secret)

### Step 2: Install the Browser Extension
Install **Aria2 Explorer** from the Chrome Web Store or Edge Add-ons store.

### Step 3: Configure RPC Host
Open the extension options:
- **Host**: `127.0.0.1` or `localhost`
- **Port**: `6800`
- **Protocol**: `WebSocket` or `HTTP`
- **Secret**: Enter your token if configured in limedl, otherwise leave blank.

Click "Test Connection". Once green, clicking downloads in your browser will automatically transfer the task to limedl!

![Browser Extension Connection Settings](../../../../assets/browser-extension-setting.png)
