# Easy Tweaks: roadmap

Triage of the feature list taken from another app's screenshots. Every tweak was checked
against the rules in CLAUDE.md: real effect, honest label (HKCU vs HKLM), fully revertible.

## Keep: per-user (HKCU), fits the current design (step 1)
Plain registry values, no admin, revertible from backups.json.

| Page | Tweaks |
|---|---|
| Input | Narrator hotkey off · Hover time (`MouseHoverTime`) |
| Debloat | Suggested apps in Start · Silent app installs · Settings suggestions · Windows tips · Lock screen tips · Start recommendations (Win11) · Advertising ID · Tailored experiences · Feedback prompts |
| Visual | Window animations · Transparency · Aero Peek · Aero Shake · Menu delay |
| Windows | Taskbar search box · Task View button · Snap Assist · Show hidden files · Show file extensions · Recent file tracking · Program tracking · Startup app delay |
| Gaming | Game DVR (`GameDVR_Enabled`) · Game Bar capture · Game Bar startup panel |

## Keep later: needs new engine work
| Needs | Items |
|---|---|
| Admin (HKLM) | GPU Hardware Scheduling · Telemetry service (privacy, not FPS) |
| Services / commands | SysMain · Search Indexer · Ultimate Performance plan · USB Selective Suspend · DNS → Cloudflare (browsing/privacy, not game ping) |
| Sliders | Cursor speed · Double-click speed · Cursor blink rate · Menu delay |
| One-off actions (label "can't be undone") | Flush DNS · Delete temp files · Disk Cleanup |

## Dropped (myth, already default, or harmful)
| Item | Why |
|---|---|
| TDR Delay 60s, Disable TDR Recovery | A GPU hang freezes the whole PC instead of recovering. No FPS gain. |
| Disable Memory Compression | Worse when RAM runs low |
| Disable Dynamic Tick | Boot config change, more power use, no proven gain |
| SystemResponsiveness, MMCSS Games, Foreground Quanta / Priority Boost | Only apps that opt into MMCSS (mostly audio); no measured gain in games |
| Mouse / Keyboard Driver Queue | Buffer size, not latency |
| Nagle, NetworkThrottlingIndex, QoS 20%, DNS cache, TTL, MaxUserPorts, TimeWait | Ping myths; games use UDP or already disable Nagle; some are server-only |
| TCP Auto-Tuning / Heuristics / Timestamps, NTFS Last-Access | Already the Windows default, or removed |
| NIC Interrupt Moderation / EEE / Flow Control / Power Saving | Different per network driver, can't revert reliably |
| Mouse Trails, Snap To Default, Max Repeat Speed, Game Mode | Already the default |
| Drag threshold, Active Window Tracking | No gaming benefit; the latter only applies to an optional feature that's off by default |
| Fullscreen Optimizations, Honor FSE | Usually neutral or better on modern Windows |
| Cortana, People Bar, News & Interests | Removed in Windows 11, or blocked for outside apps |
| Proxy auto-detect | Binary blob value; can break work/school networks |
| Polling rate, "USB Stack", Safe Mode status, Last Sync, Profile | Windows can't set polling rate; the rest has no clear meaning or backend |

## Order
0. ~~Commit + push toggle fix and Dashboard~~
1. HKCU toggles above + page structure (Visual / Windows pages, sections, Apply all / Revert all, "X of N active", active count on Dashboard)
2. Engine: tweaks that write to more than one key, apply now (`SystemParametersInfo`) instead of after sign-out, Restart Explorer button, hide tweaks that don't apply to the running Windows version (10 vs 11)
3. Sliders
4. Dashboard extras (uptime, disk, network, history chart, quick actions), Storage page, custom title bar
5. Admin tweaks. Decide first: whole app as admin (UAC prompt on every start), or ask only when an admin tweak is clicked
