//! Language of the native menu (the web view translates itself, see app/ui/src/i18n).
//!
//! The preference comes from the UI settings (`ui.layout.language`: "system", "en", "de", "zh-CN");
//! "system" follows the OS language.

use std::sync::atomic::{AtomicU8, Ordering};

static LANGUAGE: AtomicU8 = AtomicU8::new(0);

/// Resolve an explicit language or the operating system's locale.
pub fn resolve(pref: &str) -> &'static str {
    resolve_for_locale(pref, &sys_locale::get_locale().unwrap_or_default())
}

fn resolve_for_locale(pref: &str, locale: &str) -> &'static str {
    match pref {
        "de" => "de",
        "en" => "en",
        "zh-CN" => "zh-CN",
        _ => {
            let locale = locale.to_ascii_lowercase();
            if locale.starts_with("de") {
                "de"
            } else if locale.starts_with("zh") {
                "zh-CN"
            } else {
                "en"
            }
        }
    }
}

/// The preference saved in the UI settings.
pub fn saved_pref(ui: &serde_json::Value) -> String {
    ui.pointer("/layout/language")
        .and_then(|v| v.as_str())
        .unwrap_or("system")
        .to_string()
}

pub fn set(lang: &str) {
    let language = match lang {
        "de" => 1,
        "zh-CN" => 2,
        _ => 0,
    };
    LANGUAGE.store(language, Ordering::Relaxed);
}

/// Menu label in the current language (English if there is no translation).
pub fn tr(en: &str) -> &str {
    match LANGUAGE.load(Ordering::Relaxed) {
        1 => de(en).unwrap_or(en),
        2 => zh(en).unwrap_or(en),
        _ => en,
    }
}

fn de(en: &str) -> Option<&'static str> {
    Some(match en {
        "Replay Progress…" => "Replay-Fortschritt…",
        "Settings…" => "Einstellungen…",
        "New Viewer" => "Neues Fenster",
        "Load Archive…" => "Archiv laden…",
        "Recover Previous Capture…" => "Vorherige Aufzeichnung wiederherstellen…",
        "All Sessions…" => "Alle Sessions…",
        "Selected Sessions…" => "Ausgewählte Sessions…",
        "Response Body…" => "Response-Body…",
        "Request Body…" => "Request-Body…",
        "HTTP Archive (HAR)…" => "HTTP-Archiv (HAR)…",
        "SAZ Archive…" => "SAZ-Archiv…",
        "SAZ Archive with Password…" => "SAZ-Archiv mit Passwort…",
        "Packet Capture (pcap, pcapng)…" => "Paketmitschnitt (pcap, pcapng)…",
        "cURL Script…" => "cURL-Skript…",
        "WCAT Script…" => "WCAT-Skript…",
        "Snapshot Library…" => "Snapshot-Bibliothek…",
        "Internet Explorer NetXML…" => "Internet-Explorer-NetXML…",
        "Sanitized for Sharing (SAZ/HAR)…" => "Bereinigt zum Weitergeben (SAZ/HAR)…",
        "Mocks…" => "Mocks…",
        "URL" => "URL",
        "Summary" => "Zusammenfassung",
        "Headers Only" => "Nur Header",
        "Full Session" => "Vollständige Session",
        "As cURL" => "Als cURL",
        "As fetch (JavaScript)" => "Als fetch (JavaScript)",
        "As PowerShell" => "Als PowerShell",
        "As Python requests" => "Als Python requests",
        "Selected Sessions" => "Ausgewählte Sessions",
        "Unselected Sessions" => "Nicht ausgewählte Sessions",
        "All Sessions" => "Alle Sessions",
        "Red" => "Rot",
        "Blue" => "Blau",
        "Gold" => "Gold",
        "Green" => "Grün",
        "Orange" => "Orange",
        "Purple" => "Lila",
        "Unmark" => "Markierung entfernen",
        "Comment…" => "Kommentar…",
        "Find Sessions…" => "Sessions suchen…",
        "HTTPS Settings…" => "HTTPS-Einstellungen…",
        "Connect Device…" => "Gerät verbinden…",
        "Reverse Proxy…" => "Reverse Proxy…",
        "Start Browser…" => "Browser starten…",
        "Host Remapping…" => "Host-Umleitung…",
        "Open Terminal" => "Terminal öffnen",
        "Start Agent…" => "Agent starten…",
        "Before Requests" => "Vor Requests",
        "Before LLM Requests" => "Vor LLM-Requests",
        "After Responses" => "Nach Responses",
        "Off" => "Aus",
        "Mock Rules" => "Mock-Regeln",
        "Rules Script…" => "Regel-Skript…",
        "Enable Automatic Authentication" => "Automatische Anmeldung aktivieren",
        "Options…" => "Optionen…",
        "Text Tools…" => "Text-Werkzeuge…",
        "Composer" => "Composer",
        "Compare Captures…" => "Mitschnitte vergleichen…",
        "Plugins…" => "Plugins…",
        "Command Palette…" => "Befehlspalette…",
        "Focus Command Field" => "Befehlsfeld fokussieren",
        "Inspect" => "Inspektor",
        "Statistics" => "Statistik",
        "Filters" => "Filter",
        "Log" => "Protokoll",
        "Timeline" => "Zeitachse",
        "Navigator" => "Navigator",
        "Navigator: Groups" => "Navigator: Gruppen",
        "Navigator: Structure" => "Navigator: Struktur",
        "Diagnostics" => "Diagnose",
        "Agents" => "Agenten",
        "Tunnels (CONNECT)" => "Tunnel (CONNECT)",
        "Image Requests" => "Bild-Requests",
        "304 Not Modified" => "304 Not Modified",
        "Request Above Response" => "Request über Response",
        "Request Beside Response" => "Request neben Response",
        "Tear off Inspectors" => "Inspektoren abkoppeln",
        "Jobs" => "Aufgaben",
        "Generate 1,000 mock sessions" => "1.000 Test-Sessions erzeugen",
        "Generate 100,000 mock sessions" => "100.000 Test-Sessions erzeugen",
        "Generate 500,000 mock sessions" => "500.000 Test-Sessions erzeugen",
        "Mock traffic 5,000/s (continuous)" => "Testverkehr 5.000/s (fortlaufend)",
        "Stop mock traffic" => "Testverkehr stoppen",
        "Generate large bodies (≈1.4 GB)" => "Große Bodies erzeugen (≈1,4 GB)",
        "Generate huge bodies (≈14 GB)" => "Riesige Bodies erzeugen (≈14 GB)",
        "Performance Overlay" => "Performance-Anzeige",
        "Reload UI" => "Oberfläche neu laden",
        "Command Syntax" => "Befehlssyntax",
        "Keyboard Shortcuts" => "Tastenkürzel",
        "Coming from Fiddler Classic…" => "Umstieg von Fiddler Classic…",
        "Capture Traffic" => "Verkehr aufzeichnen",
        "Quena" => "Quena",
        "File" => "Datei",
        "Save" => "Speichern",
        "Import Sessions" => "Sessions importieren",
        "Export Sessions" => "Sessions exportieren",
        "Edit" => "Bearbeiten",
        "Unlock and Edit" => "Entsperren und bearbeiten",
        "Breakpoint Rules…" => "Haltepunktregeln…",
        "Copy Session" => "Session kopieren",
        "Remove" => "Entfernen",
        "Mark" => "Markieren",
        "Capture" => "Aufzeichnung",
        "Breakpoints" => "Haltepunkte",
        "Tools" => "Werkzeuge",
        "View" => "Ansicht",
        "Hide in List" => "In Liste ausblenden",
        "Developer" => "Entwickler",
        "Help" => "Hilfe",
        "About Quena" => "Über Quena",
        "Exit" => "Beenden",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    /// The string literal following `after` (skipping whitespace, commas and one id literal
    /// when `skip_id`).
    fn labels(src: &str, after: &str, skip_id: bool) -> Vec<String> {
        let mut out = vec![];
        for (i, _) in src.match_indices(after) {
            let mut rest = &src[i + after.len()..];
            let mut lit = || {
                let start = rest.find('"')? + 1;
                let len = rest[start..].find('"')?;
                let l = rest[start..start + len].to_string();
                rest = &rest[start + len + 1..];
                Some(l)
            };
            if skip_id {
                lit();
            }
            if let Some(l) = lit() {
                out.push(l);
            }
        }
        out
    }

    /// Every label in menu.rs has a German translation.
    #[test]
    fn menu_is_translated() {
        let src = include_str!("menu.rs");
        let mut all = labels(src, "item(app, ", true);
        all.extend(labels(src, "CheckMenuItem::with_id(app, ", true));
        all.extend(labels(src, "Submenu::with_items(", false));
        all.extend(labels(src, "PredefinedMenuItem::quit(app, Some(tr(", false));
        assert!(all.len() > 80, "found only {} labels", all.len());
        let missing: Vec<_> = all
            .iter()
            .filter(|l| super::de(l).is_none() || super::zh(l).is_none())
            .collect();
        assert!(missing.is_empty(), "untranslated menu labels: {missing:?}");
    }
}

fn zh(en: &str) -> Option<&'static str> {
    Some(match en {
        "Replay Progress…" => "重发进度…",
        "Settings…" => "设置…",
        "New Viewer" => "新建查看窗口",
        "Load Archive…" => "打开归档…",
        "Recover Previous Capture…" => "恢复上次抓包…",
        "All Sessions…" => "全部会话…",
        "Selected Sessions…" => "选中的会话…",
        "Response Body…" => "响应体…",
        "Request Body…" => "请求体…",
        "HTTP Archive (HAR)…" => "HTTP 归档（HAR）…",
        "SAZ Archive…" => "SAZ 归档…",
        "SAZ Archive with Password…" => "加密 SAZ 归档…",
        "Packet Capture (pcap, pcapng)…" => "数据包文件（pcap、pcapng）…",
        "cURL Script…" => "cURL 脚本…",
        "WCAT Script…" => "WCAT 脚本…",
        "Snapshot Library…" => "快照库…",
        "Internet Explorer NetXML…" => "Internet Explorer NetXML…",
        "Sanitized for Sharing (SAZ/HAR)…" => "脱敏后分享（SAZ/HAR）…",
        "Mocks…" => "模拟响应…",
        "URL" => "URL",
        "Summary" => "摘要",
        "Headers Only" => "仅头部",
        "Full Session" => "完整会话",
        "As cURL" => "cURL 命令",
        "As fetch (JavaScript)" => "fetch 代码（JavaScript）",
        "As PowerShell" => "PowerShell 代码",
        "As Python requests" => "Python requests 代码",
        "Selected Sessions" => "选中的会话",
        "Unselected Sessions" => "未选中的会话",
        "All Sessions" => "全部会话",
        "Red" => "红色",
        "Blue" => "蓝色",
        "Gold" => "金色",
        "Green" => "绿色",
        "Orange" => "橙色",
        "Purple" => "紫色",
        "Unmark" => "取消标记",
        "Comment…" => "备注…",
        "Find Sessions…" => "查找会话…",
        "HTTPS Settings…" => "HTTPS 设置…",
        "Connect Device…" => "连接设备…",
        "Reverse Proxy…" => "反向代理…",
        "Start Browser…" => "启动浏览器…",
        "Host Remapping…" => "主机映射…",
        "Open Terminal" => "打开终端",
        "Start Agent…" => "启动智能体…",
        "Before Requests" => "发送请求前",
        "Before LLM Requests" => "发送大模型请求前",
        "After Responses" => "返回响应前",
        "Off" => "关闭",
        "Mock Rules" => "模拟规则",
        "Rules Script…" => "规则脚本…",
        "Enable Automatic Authentication" => "启用自动认证",
        "Options…" => "选项…",
        "Text Tools…" => "文本工具…",
        "Composer" => "请求构造器",
        "Compare Captures…" => "比较抓包…",
        "Plugins…" => "插件…",
        "Command Palette…" => "命令面板…",
        "Focus Command Field" => "聚焦命令栏",
        "Inspect" => "会话详情",
        "Statistics" => "统计",
        "Filters" => "筛选器",
        "Log" => "日志",
        "Timeline" => "时间轴",
        "Navigator" => "导航器",
        "Navigator: Groups" => "导航器：分组",
        "Navigator: Structure" => "导航器：结构",
        "Diagnostics" => "诊断",
        "Agents" => "智能体",
        "Tunnels (CONNECT)" => "隧道（CONNECT）",
        "Image Requests" => "图片请求",
        "304 Not Modified" => "304 Not Modified（未修改）",
        "Request Above Response" => "请求在上，响应在下",
        "Request Beside Response" => "请求与响应并排",
        "Tear off Inspectors" => "在独立窗口查看会话详情",
        "Jobs" => "后台任务",
        "Generate 1,000 mock sessions" => "生成 1,000 条模拟会话",
        "Generate 100,000 mock sessions" => "生成 100,000 条模拟会话",
        "Generate 500,000 mock sessions" => "生成 500,000 条模拟会话",
        "Mock traffic 5,000/s (continuous)" => "持续生成模拟流量（每秒 5,000 条）",
        "Stop mock traffic" => "停止模拟流量",
        "Generate large bodies (≈1.4 GB)" => "生成大体积正文（约 1.4 GB）",
        "Generate huge bodies (≈14 GB)" => "生成超大正文（约 14 GB）",
        "Performance Overlay" => "性能监视浮层",
        "Reload UI" => "重新加载界面",
        "Command Syntax" => "命令语法",
        "Keyboard Shortcuts" => "键盘快捷键",
        "Coming from Fiddler Classic…" => "Fiddler Classic 用户指南…",
        "Capture Traffic" => "抓包",
        "Quena" => "Quena",
        "File" => "文件",
        "Save" => "保存",
        "Import Sessions" => "导入会话",
        "Export Sessions" => "导出会话",
        "Edit" => "编辑",
        "Unlock and Edit" => "解锁并编辑",
        "Breakpoint Rules…" => "断点规则…",
        "Copy Session" => "复制会话",
        "Remove" => "移除",
        "Mark" => "标记",
        "Capture" => "抓包",
        "Breakpoints" => "断点",
        "Tools" => "工具",
        "View" => "视图",
        "Hide in List" => "在列表中隐藏",
        "Developer" => "开发者",
        "Help" => "帮助",
        "About Quena" => "关于 Quena",
        "Exit" => "退出",
        _ => return None,
    })
}

#[cfg(test)]
mod chinese_tests {
    use super::*;

    #[test]
    fn chinese_systems_and_explicit_preferences() {
        assert_eq!(resolve_for_locale("system", "zh_CN"), "zh-CN");
        assert_eq!(resolve_for_locale("system", "zh-Hans-CN"), "zh-CN");
        assert_eq!(resolve_for_locale("en", "zh_CN"), "en");
        assert_eq!(resolve_for_locale("de", "zh_CN"), "de");
        assert_eq!(resolve_for_locale("zh-CN", "en_US"), "zh-CN");
        assert_eq!(resolve_for_locale("system", "de_DE"), "de");
        assert_eq!(resolve_for_locale("system", "fr_FR"), "en");
    }

    #[test]
    fn chinese_native_menu_and_fallback() {
        assert_eq!(zh("Settings…"), Some("设置…"));
        assert_eq!(zh("Capture"), Some("抓包"));
        assert_eq!(zh("Unknown menu item"), None);
    }
}
