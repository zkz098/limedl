---
title: One-Click Browser Handoff
description: Use Aria2 RPC and browser extensions to send web downloads to limedl, forwarding Cookie, Referer, and User-Agent headers
---

# One-Click Browser Handoff

## Who this is for

- You click download links in Chrome, Edge, or Firefox every day;
- You want large files handled by limedl while small files stay with the browser;
- Your cloud-drive or forum links need session cookies and anti-hotlink headers.

The flow: a browser extension intercepts downloads and pushes tasks to the local limedl through **Aria2 JSON-RPC 2.0**.

```text
Browser extension (Aria2 Explorer / Camtd / userscript)
        │  aria2.addUri / addTorrent (HTTP / WebSocket)
        ▼
   limedl built-in RPC (127.0.0.1:6800)
        ▼
   HTTP / BitTorrent engine
```

---

## Three-step setup

### 1. Enable limedl RPC

Open **Settings → Aria2 RPC**:

- Enable **Aria2 RPC** (off by default on a fresh install);
- Confirm port `6800`, or choose another port;
- Set an **RPC secret token**;
- Keep the listen address on `127.0.0.1`; never expose it directly to the internet.

### 2. Install a browser extension

**Aria2 Explorer** is recommended (Chrome / Edge / Firefox); Camtd and userscripts also work. See [Browser Integration](/en/ecosystem/browser/).

### 3. Fill in the connection

| Setting | Value |
| :--- | :--- |
| Host | `127.0.0.1` |
| Port | `6800` (match limedl) |
| Protocol | HTTP or WebSocket |
| Secret | The same token as limedl; leave empty if none |

Click “Test connection” and confirm it succeeds.

---

## Tuning interception rules

- **Minimum file size**: 10–20 MB avoids popups for images and small PDFs;
- **Extension allowlist**: `zip, rar, 7z, tar, gz, iso, exe, msi, mp4, mkv, deb, dmg, pkg`, etc.;
- **Temporary bypass/takeover**: Aria2 Explorer supports `Alt` to force the native browser download and `Shift` to force limedl.

---

## Cookies, Referer, and anti-hotlinking

The extension sends the current tab's `Cookie`, `Referer`, and `User-Agent` as a `header` array with `aria2.addUri`:

```json
{
  "header": [
    "Cookie: PHPSESSID=xxxx; auth_token=yyyy",
    "Referer: https://example.com/download-page",
    "User-Agent: Mozilla/5.0 ..."
  ]
}
```

limedl attaches them to chunk requests, which lets downloads pass session and anti-hotlink checks. Actual behavior depends on whether the extension/userscript captures those headers correctly; some cloud drives also need dedicated parsing scripts.

---

## Security and troubleshooting

### Security

- RPC is **off by default** and listens only on `127.0.0.1`;
- Set a token; the desktop app also supports per-client tokens;
- Never map port 6800 directly to the internet;
- CORS defaults to local origins; add only what you need and avoid leaving `*` enabled.

### Connection failures

1. Confirm limedl is running and RPC is enabled;
2. Confirm the port matches and the local firewall is not blocking it;
3. Confirm the secret matches;
4. Try switching the protocol between HTTP and WebSocket;
5. Check the `logs/` directory under the limedl data directory.

### 401 / 403 responses

- `401`: token mismatch; check `token:<secret>`;
- `403`: anti-bot or anti-hotlink protection; check UA, Cookie, and Referer, or use another mirror.

---

## When not to use it

- Thousands of tiny files: the browser is faster; use the minimum-size filter;
- Private downloads that require dynamic page signatures: depends on the extension/userscript; limedl has no site-specific parser;
- Environments where local processes must not reach the browser: do not enable RPC.

---

## Related documentation

- [Browser Integration](/en/ecosystem/browser/): extension links and illustrated setup.
- [Aria2 RPC](/en/ecosystem/aria2-rpc/): methods, authentication, CORS, and client examples.
- [Security & Verification](/en/security/): release and update security boundaries.
- [FAQ](/en/faq): RPC exposure risks, 403s, and platform limits.
