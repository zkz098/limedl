# Injects native Slint `Tooltip` elements into settings field labels and
# emits the matching po translation block. Tooltip copy mirrors the Tauri
# (Vue) edition: en-US text as msgid, zh-CN text as msgstr.
$ErrorActionPreference = "Stop"
$m = [ordered]@{
    "Theme Mode (system/light/dark):" = @("Follow system switches automatically from the current Windows app theme.", "跟随系统会根据 Windows 当前的应用主题自动切换。")
    "Background Opacity Preset (default/acrylic/frosted):" = @("Controls transparency and background blur for app panels and dialogs.", "影响应用面板与模态框的透明度和背景模糊程度。")
    "Show Detail Panel" = @("Show detailed fields like URL, checksum, threads in the download inspector.", "在下载检查器中显示 URL、校验和、线程等详细字段。")
    "Launch on Startup" = @("Start Limedl automatically when you log in. The app will start minimized to the system tray.", "登录系统时自动启动 Limedl，启动后将最小化到系统托盘。")
    "Double-click Completed Task (none/open_file/open_in_explorer/open_download_dir):" = @("Action to perform when double-clicking a completed download task.", "设置双击已完成下载任务时的行为。")
    "Double-click Uncompleted Task (none/toggle_pause_resume):" = @("Action to perform when double-clicking an uncompleted download task.", "设置双击未完成下载任务时的行为。")
    "Default Download Folder:" = @("New tasks will prefill this folder, and you can still override it per task.", "新建任务时会自动带入该目录，你仍然可以在任务里临时改掉。")
    "Default Checksum (blake3/sha256/xxh3_128/none/sha1):" = @("New tasks no longer show checksum mode separately. This setting is used globally.", "新建任务中不再单独显示校验方式，统一使用这里的设置。")
    "Auto-detect SHA-256" = @("Automatically probe for matching .sha256 or parent SHA256SUMS files when downloading HTTP links, and verify after completion.", "下载 HTTP 链接时自动探测同名 .sha256 或目录下的 SHA256SUMS 校验文件，完成后自动核对。")
    "Default User-Agent (empty for Chrome 124):" = @("New HTTP tasks prefill this UA, and each task can override it.", "新建 HTTP 任务会自动带入该 UA，也可以在任务里单独覆盖。")
    "Global Bandwidth Limit (0 for unlimited):" = @("Limit total download speed across all active tasks. 0 means unlimited.", "限制所有活跃任务的总下载速度。0 表示不限速。")
    "Proxy Mode (disabled/system/manual):" = @("Supports common HTTP / HTTPS / SOCKS proxy URLs. Enter a full URL.", "支持常见 HTTP / HTTPS / SOCKS 代理地址，按完整 URL 填写。")
    "Adaptive Profile (conservative/balanced/aggressive):" = @("Conservative limits thread growth, Balanced trades overhead for speed, and Aggressive prioritizes download speed.", "保守更偏节制线程，平衡兼顾开销与速度，激进优先追求下载速度。")
    "Chunk Strategy (adaptive/fixed):" = @("Auto-adjust chunk strategy based on network conditions", "根据网络状况自动调整分块策略")
    "Min Threads per Task (0-64):" = @("Set to 0 to use 50% of the max thread count automatically. The adaptive algorithm never goes below this once reached.", "设为 0 则自动取最大线程数的 50%。到达最低值后自适应算法不会继续减线程。")
    "Enable DHT" = @("DHT discovers peers from the distributed network. Changes apply to the BT session after restart.", "DHT 会从分布式网络发现 Peer；修改后会在下次启动的 BT 会话中生效。")
    "Listen Port (empty for random):" = @("Leave empty for auto-assignment. 1024–65535", "留空由系统自动分配。1024–65535")
    "Max Peers per Torrent:" = @("Maximum peer connections per torrent.", "每个 torrent 的最大 peer 连接数。")
    "Tracker Subscription URL:" = @("Download a TXT file from this URL, one tracker URL per line. Updating refreshes the list below; save settings afterward.", "从该地址下载 TXT，每行一个 Tracker URL。点击更新只会刷新下方列表，仍需保存设置。")
    "Encryption Mode (enabled/disabled/forced):" = @("MSE/PE protocol encryption. Forced mode may reject unencrypted connections. irontide only.", "MSE/PE 协议加密。强制模式可能拒绝未加密连接。仅 irontide。")
    "Pre-allocation Mode (none/full):" = @("Full preallocation reduces disk fragmentation but is slower on task creation. irontide only.", "完全预分配可减少磁盘碎片，但创建任务较慢。仅 irontide。")
    "Max Active Downloads:" = @("Maximum number of auto-managed active downloads", "自动管理的最大活跃下载任务数")
    "Max Active Seeds:" = @("Maximum number of auto-managed active seeds", "自动管理的最大活跃做种任务数")
    "Max Torrents:" = @("Maximum total number of torrents allowed", "允许添加的 Torrent 总数上限")
    "Active Limit:" = @("Hard limit on total active torrents (downloading + seeding + checking)", "同时活跃（下载+做种+校验）的硬上限")
    "Global Download Rate Limit:" = @("0 = unlimited", "0 = 不限制")
    "Global Upload Rate Limit:" = @("0 = unlimited", "0 = 不限制")
    "Pause Seeding upon Reaching Limit" = @("When either the uploaded amount or share ratio limit is reached, the matching BT task is paused.", "满足上传量上限或分享率上限任一条件后，会暂停对应 BT 任务。")
    "Seeding Upload Limit:" = @("0 or empty means no limit", "填 0 或留空表示不限速")
    "Share Ratio Limit (0 for unlimited):" = @("For example, 2 pauses after uploaded bytes reach 2x downloaded bytes. Use 0 to disable the ratio limit.", "例如 2 表示上传量达到已下载量的 2 倍后暂停；填 0 表示不按倍数限制。")
    "Grace Period:" = @("Seconds a peer must be unchoked before it can be flagged, avoiding penalising slow-start peers.", "对等端需被放开上传这么久后才可能被标记，避免误伤刚接入的慢启动对等端。")
    "Share Ratio Threshold (0-1):" = @("Peers sending back less than this share of what they take are treated as leechers. 0 disables the ratio check.", "回传数据低于其取得数据该比例的 peer 将被视为吸血者；填 0 关闭比例判断。")
    "Ban Duration:" = @("Seconds a banned peer stays banned before it is unbanned and may reconnect.", "被封禁对等端需等待的秒数，到期后自动解封可重新连接。")
    "Action (ban/limit_slots):" = @("Ban disconnects and blacklists offending peers for a while; Limit Slots caps how many peers a torrent uploads to at once.", "封禁：断开并暂时拉黑违规对等端；限制上传槽位：当 torrent 有吸血对等端时限制同时上传的槽位数。")
    "Max Upload Slots in Limit Mode (1-64):" = @("When a torrent has detected leechers, its concurrent upload slots are capped at this value.", "当 torrent 检测到吸血对等端时，其并发上传槽位将限制为该值。")
    "Enable Anti-Leech Detection" = @("Periodically identify peers that download from you without giving data back, then enforce the selected action.", "定期识别只下载却不回传数据的对等端，并按所选策略处理。")
    "Enable IP Blocklist" = @("Load a peer IP blocklist into the session. Blocked ranges (e.g. known leeching IDC/networks) are refused on connection.", "将会话的 IP 过滤设为黑名单文件中的网段，被封段的（如已知吸血 IDC/网络）连接将被拒绝。")
    "Blocklist Path:" = @("Path to a blocklist file. Use an eMule .dat file or a P2P plaintext file (one CIDR per line); format is chosen by file extension.", "填写黑名单文件路径。支持 eMule .dat 或 P2P 明文（每行一个 CIDR）格式，按文件扩展名自动选择解析器。")
    "Seed Choking (fastest_upload/round_robin/anti_leech):" = @("How peers are ranked while seeding. Anti-Leech prefers seeding peers that contribute the most back.", "做种时对 peer 的排序方式。反吸血优先回报最高的做种对等端。")
    "Choking Algorithm (fixed_slots/rate_based):" = @("How unchoke slots are determined per torrent. Fixed keeps a constant slot count; Rate-based auto-adjusts.", "每个 torrent 的 unchoke 槽位如何确定：固定槽位数 或 按速率自动调整。")
    "Max Upload Slots per Torrent:" = @("Maximum concurrent unchoke slots per torrent.", "每个 torrent 的最大并发 unchoke 槽位数。")
    "Smart Ban Threshold (1-100):" = @("Hash-failure involvements before a peer is automatically banned.", "hash 校验失败的涉事次数超过该值后自动封禁该 peer。")
    "Smart Ban Parole" = @("Re-download a failed piece from an uninvolved peer to attribute the fault before striking.", "从无关 peer 重新下载失败分片以判定责任，判责前不扣分。")
    "Ban Duration after Eviction:" = @("Seconds an evicted (non-contributing) peer is blocked from reconnecting.", "被驱逐（不贡献数据的）peer 需等待的封禁秒数，到期后可重新连接。")
    "No Data Contribution Timeout:" = @("Seconds without receiving piece data before a peer is disconnected. 0 disables this safeguard.", "超过该秒数未收到分片数据的 peer 将被断开；填 0 关闭此保护。")
    "HDD Double-Buffer Optimization" = @("Enable double-buffer pool optimization for mechanical hard drives to improve write performance. Not needed for SSDs.", "为机械硬盘启用双缓冲池优化，提升写入性能。SSD 不需要此功能。")
    "Buffer Pool Limit:" = @("Maximum memory used to buffer HDD downloads. A larger buffer turns more random writes into sequential writes.", "HDD 下载时用于缓冲的内存上限。更大的缓冲区可将更多随机写入转化为顺序写入。")
    "Game Mode Buffer:" = @("Buffer limit when game mode is active, reducing impact on gaming performance.", "游戏模式激活时的缓冲区上限，降低对游戏性能的影响。")
    "HDD Max Parallel:" = @("Maximum number of HDD downloads that can use the buffer simultaneously. Each gets an equal share of the buffer pool.", "可同时使用缓冲区的 HDD 下载数量上限。每个下载均分缓冲池。")
    "Game Mode Max Parallel:" = @("Maximum parallel HDD downloads when game mode is active.", "游戏模式激活时的最大并行 HDD 下载数。")
    "Log File Path:" = @("Supports absolute or relative paths. Leave empty to write to logs/limedl.log under app data.", "支持绝对路径或相对路径；留空时自动写入应用数据目录下的 logs/limedl.log。")
    "Enable Aria2 RPC Service" = @("Start a local Aria2 RPC-compatible server, allowing Chrome download extensions to send downloads via the Aria2 protocol.", "在本地启动兼容 Aria2 RPC 的服务端，允许 Chrome 下载插件通过 Aria2 协议发起下载。")
    "RPC Port (1-65535):" = @("Default 6800. Make sure the port is available.", "默认 6800。确保端口未被其他程序占用。")
    "RPC Secret (empty for no authentication):" = @("Leave empty to allow connections without a secret. If set, configure the same token in your browser extension.", "留空允许无密钥连接。设置后需在插件中配置相同 token。")
}
$file = "D:\limedl\crates\limedl-native\ui\components\settings_dialog.slint"
$text = [System.IO.File]::ReadAllText($file)
$injected = 0
$missed = [System.Collections.Generic.List[string]]::new()

foreach ($entry in $m.GetEnumerator()) {
    $label = $entry.Key
    $en = $entry.Value[0]
    $zh = $entry.Value[1]
    $labelEsc = [regex]::Escape($label)
    $q = '"'
    $pattern = '(?m)^([ \t]*)Text \{ text: @tr\(' + $q + $labelEsc + $q + '\); (.*) \}$'
    $mm = [regex]::Match($text, $pattern)
    if (-not $mm.Success) { $missed.Add($label); continue }
    $indent = $mm.Groups[1].Value
    $rest = $mm.Groups[2].Value
    $replacement = $indent + 'Text { text: @tr(' + $q + $label + $q + '); ' + $rest + "`n" `
        + $indent + '    Tooltip { text: @markdown(@tr(' + $q + $en + $q + ')); } }'
    $text = $text.Remove($mm.Index, $mm.Length).Insert($mm.Index, $replacement)
    $injected++
}
[System.IO.File]::WriteAllText($file, $text, [System.Text.UTF8Encoding]::new($false))
Write-Host "injected: $injected / $($m.Count)"
if ($missed.Count) { Write-Host "NOT FOUND:"; $missed | ForEach-Object { "  [$_]" } }

# emit po block (msgid = en hint, msgstr = zh hint)
$po = New-Object System.Text.StringBuilder
foreach ($entry in $m.GetEnumerator()) {
    [void]$po.AppendLine('')
    [void]$po.AppendLine('msgid ' + $q + $entry.Value[0] + $q)
    [void]$po.AppendLine('msgstr ' + $q + $entry.Value[1] + $q)
}
[System.IO.File]::WriteAllText("$env:TEMP\tooltips-po-block.txt", $po.ToString(), [System.Text.UTF8Encoding]::new($false))
Write-Host "po block written to $env:TEMP\tooltips-po-block.txt"