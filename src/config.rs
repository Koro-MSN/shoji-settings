//! ShojiWM の設定ファイル（~/.config/shojiwm/src/index.tsx）の読み書き。
//!
//! index.tsx は TypeScript のプログラムなので、全体を書き換えることはせず、
//! 既知のブロックの中の「値」だけを書き換える。変更されていない項目は一切触らない。
//! 形が想定と違う場合は None を返し、呼び出し側は保存を中止する（勝手に壊さない）。
//!
//! 起動コマンドは専用ファイル autostart.ts を丸ごと管理する（このアプリが生成する
//! ファイルなので全面書き換えでよい。保存前にバックアップを取る）。

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------- モデル ----------

#[derive(Clone, Debug, PartialEq)]
pub struct Touchpad {
    pub present: bool,
    pub tap_to_click: bool,
    pub natural_scroll: bool,
    pub scroll_method: String,
    pub scroll_factor: f32,
    pub disable_while_typing: bool,
    pub click_method: Option<String>,
    pub tap_button_map: Option<String>,
}

impl Default for Touchpad {
    fn default() -> Self {
        Self {
            present: false,
            tap_to_click: true,
            natural_scroll: false,
            scroll_method: "twoFinger".to_string(),
            scroll_factor: 1.0,
            disable_while_typing: true,
            click_method: None,
            tap_button_map: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pointer {
    pub present: bool,
    pub accel: f32,
    pub accel_profile: String,
    pub left_handed: Option<bool>,
    pub middle_emulation: Option<bool>,
    pub natural_scroll: Option<bool>,
}

impl Default for Pointer {
    fn default() -> Self {
        Self {
            present: false,
            accel: 0.0,
            accel_profile: "adaptive".to_string(),
            left_handed: None,
            middle_emulation: None,
            natural_scroll: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hdr {
    Unset,
    Off,
    On,
    /// `{ ... }` の詳細オブジェクト形式（オン扱い。形はそのまま保持する）
    Options,
}

impl Hdr {
    pub fn is_on(self) -> bool {
        matches!(self, Hdr::On | Hdr::Options)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OutputCfg {
    pub name: String,
    pub scale: f32,
    pub vrr: Option<bool>,
    pub hdr: Hdr,
    pub direct_scanout: Option<bool>,
    pub subpixel: Option<String>,
    pub icc: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LidField {
    Absent,
    Value(String),
    /// 関数で指定されている（内容は触らない）
    Function,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lid {
    pub on_battery: LidField,
    pub on_external: LidField,
    pub internal_panel_off: bool,
}

impl Default for Lid {
    fn default() -> Self {
        Self {
            on_battery: LidField::Absent,
            on_external: LidField::Absent,
            internal_panel_off: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CornerAction {
    None,
    Switcher,
    WorkspaceNext,
    WorkspacePrev,
    Lock,
}

/// [左上, 右上, 左下, 右下]
pub type Corners = [CornerAction; 4];
pub const CORNER_KEYS: [&str; 4] = ["top-left", "top-right", "bottom-left", "bottom-right"];
pub const CORNER_LABELS: [&str; 4] = ["左上", "右上", "左下", "右下"];

#[derive(Clone, Debug, PartialEq)]
pub struct WmConfig {
    pub cursor_theme: String,
    pub cursor_size: f32,
    pub kb_layout: String,
    pub kb_options: String,
    pub kb_repeat_rate: f32,
    pub kb_repeat_delay: f32,
    pub touchpad: Touchpad,
    pub pointer: Pointer,
    pub outputs: Vec<OutputCfg>,
    pub frame_pacing: String,
    pub frame_pacing_present: bool,
    pub fps_initial: bool,
    pub fps_present: bool,
    pub lid: Lid,
    /// None = マーカーが無い config（ホットコーナーは編集不可）
    pub hotcorners: Option<Corners>,
}

impl Default for WmConfig {
    fn default() -> Self {
        Self {
            cursor_theme: "Bibata-Modern-Ice".to_string(),
            cursor_size: 24.0,
            kb_layout: "us".to_string(),
            kb_options: String::new(),
            kb_repeat_rate: 60.0,
            kb_repeat_delay: 250.0,
            touchpad: Touchpad::default(),
            pointer: Pointer::default(),
            outputs: Vec::new(),
            frame_pacing: "throughput".to_string(),
            frame_pacing_present: false,
            fps_initial: false,
            fps_present: false,
            lid: Lid::default(),
            hotcorners: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AutoEntry {
    pub name: String,
    pub command: String,
    pub service: bool,
}

// ---------- パス ----------

fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("shojiwm").join("src")
}

pub fn config_path() -> PathBuf {
    config_dir().join("index.tsx")
}

pub fn autostart_path() -> PathBuf {
    config_dir().join("autostart.ts")
}

pub fn read_config_text() -> Option<String> {
    fs::read_to_string(config_path()).ok()
}

pub fn read_autostart_text() -> Option<String> {
    fs::read_to_string(autostart_path()).ok()
}

// ---------- 汎用スキャナ ----------

/// `from` 以降で最初に現れる `open` から、対応する `close` までの範囲（両端含む）。
/// 文字列の中の括弧は無視する。
fn pair_range(text: &str, from: usize, open: u8, close: u8) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let start = text[from..].find(open as char)? + from;
    let mut depth = 0i32;
    let mut i = start;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'"' | b'\'' | b'`' => quote = Some(b),
            _ if b == open => depth += 1,
            _ if b == close => {
                depth -= 1;
                if depth == 0 {
                    return Some((start, i));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn block_range(text: &str, from: usize) -> Option<(usize, usize)> {
    pair_range(text, from, b'{', b'}')
}

/// ブロック（`{...}` の文字列）の中で、深さ1にある `key:` の値のバイト範囲を返す。
fn field_range(block: &str, key: &str) -> Option<(usize, usize)> {
    let bytes = block.as_bytes();
    let mut depth = 0i32;
    let mut i = 0usize;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'"' | b'\'' | b'`' => {
                quote = Some(b);
                i += 1;
            }
            b'{' | b'[' | b'(' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' | b')' => {
                depth -= 1;
                i += 1;
            }
            _ if depth == 1 && (b.is_ascii_alphabetic() || b == b'_') => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let ident = &block[start..i];
                // キーとして一致し、続いて ':' が来るものだけを対象にする
                let mut j = i;
                while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                    j += 1;
                }
                if ident == key && j < bytes.len() && bytes[j] == b':' {
                    let mut v = j + 1;
                    while v < bytes.len() && (bytes[v] == b' ' || bytes[v] == b'\t') {
                        v += 1;
                    }
                    // 値の終わり：深さ0の ',' か、ブロック末尾の '}' の手前
                    let mut d = 0i32;
                    let mut k = v;
                    let mut q2: Option<u8> = None;
                    while k < bytes.len() {
                        let c = bytes[k];
                        if let Some(q) = q2 {
                            if c == b'\\' {
                                k += 2;
                                continue;
                            }
                            if c == q {
                                q2 = None;
                            }
                            k += 1;
                            continue;
                        }
                        match c {
                            b'"' | b'\'' | b'`' => q2 = Some(c),
                            b'{' | b'[' | b'(' => d += 1,
                            b'}' | b']' | b')' => {
                                if d == 0 {
                                    break;
                                }
                                d -= 1;
                            }
                            b',' if d == 0 => break,
                            _ => {}
                        }
                        k += 1;
                    }
                    let mut end = k;
                    while end > v && (bytes[end - 1] == b' ' || bytes[end - 1] == b'\t') {
                        end -= 1;
                    }
                    return Some((v, end));
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    None
}

fn field_raw(block: &str, key: &str) -> Option<String> {
    let (s, e) = field_range(block, key)?;
    Some(block[s..e].to_string())
}

fn raw_str_field(block: &str, key: &str) -> Option<String> {
    let raw = field_raw(block, key)?;
    let raw = raw.trim();
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        Some(raw[1..raw.len() - 1].to_string())
    } else {
        None
    }
}

fn raw_bool_field(block: &str, key: &str) -> Option<bool> {
    match field_raw(block, key)?.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn raw_num_field(block: &str, key: &str) -> Option<f32> {
    field_raw(block, key)?.trim().parse().ok()
}

/// ブロック先頭の既存フィールド行のインデント。無ければ2スペース。
fn first_field_indent(block: &str) -> String {
    for line in block.lines().skip(1) {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('}') {
            continue;
        }
        let indent: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        if !indent.is_empty() {
            return indent;
        }
    }
    "  ".to_string()
}

/// ブロック内の値を書き換える。raw が None なら行ごと消す。
/// キーが無く raw が Some なら、先頭に1行挿入する。
fn set_field(block: &str, key: &str, raw: Option<&str>) -> Option<String> {
    match (field_range(block, key), raw) {
        (Some((s, e)), Some(raw)) => {
            let mut out = String::with_capacity(block.len() + raw.len());
            out.push_str(&block[..s]);
            out.push_str(raw);
            out.push_str(&block[e..]);
            Some(out)
        }
        (Some((s, _)), None) => {
            // 行頭（インデント含む）から、末尾の ',' と改行までを消す
            let line_start = block[..s].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let mut end = block[s..]
                .find('\n')
                .map(|i| s + i + 1)
                .unwrap_or(block.len());
            if end == block.len() && block.ends_with('}') {
                end = block.len() - 1;
            }
            let mut out = String::with_capacity(block.len());
            out.push_str(&block[..line_start]);
            out.push_str(&block[end..]);
            Some(out)
        }
        (None, Some(raw)) => {
            let indent = first_field_indent(block);
            let mut out = String::with_capacity(block.len() + raw.len() + key.len() + 8);
            out.push_str(&block[..1]);
            out.push('\n');
            out.push_str(&indent);
            out.push_str(key);
            out.push_str(": ");
            out.push_str(raw);
            out.push(',');
            out.push_str(&block[1..]);
            Some(out)
        }
        (None, None) => Some(block.to_string()),
    }
}

fn fmt_num(v: f32) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn raw_num(v: f32) -> String {
    fmt_num(v)
}

fn raw_bool(v: bool) -> String {
    if v {
        "true".to_string()
    } else {
        "false".to_string()
    }
}

fn raw_str(v: &str) -> String {
    let mut out = String::with_capacity(v.len() + 2);
    out.push('"');
    for c in v.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// サブブロック（`key: { ... }`）の範囲を、親ブロック内での相対で返す。
fn sub_block_range(block: &str, key: &str) -> Option<(usize, usize)> {
    let (s, _) = field_range(block, key)?;
    if block.as_bytes().get(s) != Some(&b'{') {
        return None;
    }
    block_range(block, s)
}

// ---------- index.tsx の読み取り ----------

fn cursor_block_range(text: &str) -> Option<(usize, usize)> {
    let m = text.find("COMPOSITOR.cursor.configure")?;
    block_range(text, m)
}

fn input_global_range(text: &str) -> Option<(usize, usize)> {
    let m = text.find("input.global")?;
    block_range(text, m)
}

/// `bindLidPolicy({ ... })` のブロック（import 文と区別するため `({` まで含める）。
fn lid_block_range(text: &str) -> Option<(usize, usize)> {
    let m = text.find("bindLidPolicy({")?;
    block_range(text, m)
}

fn parse_lid_field(block: &str, key: &str) -> LidField {
    match field_raw(block, key) {
        None => LidField::Absent,
        Some(raw) => {
            let raw = raw.trim();
            if raw.starts_with('"') {
                LidField::Value(raw[1..raw.len().saturating_sub(1)].to_string())
            } else {
                LidField::Function
            }
        }
    }
}

const HOT_BEGIN: &str = "// [shoji-settings] hotcorners begin";
const HOT_END: &str = "// [shoji-settings] hotcorners end";

/// ホットコーナーの管理領域 (begin行の終わり, end行の始まり) を返す。
fn hot_region(text: &str) -> Option<(usize, usize)> {
    let b = text.find(HOT_BEGIN)?;
    let begin_end = text[b..].find('\n').map(|i| b + i + 1)?;
    let e = text[begin_end..].find(HOT_END)? + begin_end;
    Some((begin_end, e))
}

fn parse_hotcorners(text: &str) -> Option<Corners> {
    let (s, e) = hot_region(text)?;
    let region = &text[s..e];
    let mut corners: Corners = [CornerAction::None; 4];
    for (idx, key) in CORNER_KEYS.iter().enumerate() {
        let needle = format!("bindHotCorner(\"settings-{key}\"");
        if let Some(pos) = region.find(&needle) {
            let rest = &region[pos..];
            let end = rest.find(");").map(|i| i + 2).unwrap_or(rest.len());
            let stmt = &rest[..end];
            corners[idx] = if stmt.contains("WINDOW_SWITCHER.open") {
                CornerAction::Switcher
            } else if stmt.contains("switchWorkspace(1)") {
                CornerAction::WorkspaceNext
            } else if stmt.contains("switchWorkspace(-1)") {
                CornerAction::WorkspacePrev
            } else if stmt.contains("msg lock") {
                CornerAction::Lock
            } else {
                CornerAction::None
            };
        }
    }
    Some(corners)
}

fn hotcorner_line(key: &str, action: CornerAction) -> Option<String> {
    let body = match action {
        CornerAction::None => return None,
        CornerAction::Switcher => {
            "WINDOW_SWITCHER.open(HYBRID_WINDOW_MANAGER.getCurrentMonitorName());".to_string()
        }
        CornerAction::WorkspaceNext => {
            "HYBRID_WINDOW_MANAGER.switchWorkspace(1); scheduleWorkspaceBroadcast();".to_string()
        }
        CornerAction::WorkspacePrev => {
            "HYBRID_WINDOW_MANAGER.switchWorkspace(-1); scheduleWorkspaceBroadcast();".to_string()
        }
        CornerAction::Lock => {
            "COMPOSITOR.process.spawn({ command: `${SHELL} msg lock` });".to_string()
        }
    };
    Some(format!(
        "COMPOSITOR.pointer.bindHotCorner(\"settings-{key}\", \"{key}\", () => {{ {body} }}, {{ delay: 150 }});"
    ))
}

fn parse_outputs(text: &str) -> Vec<OutputCfg> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(rel) = text[pos..].find("display[\"") {
        let at = pos + rel;
        let name_start = at + "display[\"".len();
        let Some(name_end) = text[name_start..].find("\"]").map(|i| name_start + i) else {
            break;
        };
        let name = text[name_start..name_end].to_string();
        let Some((o, c)) = block_range(text, name_end) else {
            break;
        };
        let block = &text[o..=c];
        // ドック接続時に内蔵画面を無効化する上書き（`{ mode: "disabled" }`）は
        // 本表ではないので読み飛ばす。本表のエントリは scale を必ず持つ。
        if !block.contains("scale:") {
            pos = c + 1;
            continue;
        }
        let hdr = match field_raw(block, "hdr") {
            None => Hdr::Unset,
            Some(raw) => {
                let raw = raw.trim();
                if raw == "false" {
                    Hdr::Off
                } else if raw == "true" {
                    Hdr::On
                } else if raw.starts_with('{') {
                    Hdr::Options
                } else {
                    Hdr::Unset
                }
            }
        };
        out.push(OutputCfg {
            name,
            scale: raw_num_field(block, "scale").unwrap_or(1.0),
            vrr: raw_bool_field(block, "vrr"),
            hdr,
            direct_scanout: raw_bool_field(block, "directScanout"),
            subpixel: raw_str_field(block, "subpixel"),
            icc: raw_str_field(block, "icc"),
        });
        pos = c + 1;
    }
    out
}

/// `COMPOSITOR.rendering.framePacing = "throughput";` の値の範囲。
fn frame_pacing_range(text: &str) -> Option<(usize, usize)> {
    let m = text.find("COMPOSITOR.rendering.framePacing")?;
    let rest = &text[m..];
    let eq = rest.find('=')?;
    let q1 = rest[eq..].find('"')? + eq;
    let q2 = rest[q1 + 1..].find('"')? + q1 + 1;
    Some((m + q1 + 1, m + q2))
}

/// `let fpsCounter = false;` の真偽値の範囲。
fn fps_range(text: &str) -> Option<(usize, usize)> {
    let m = text.find("let fpsCounter =")?;
    let rest = &text[m..];
    if let Some(rel) = rest.find("false") {
        if rest.find("true").map(|t| t > rel).unwrap_or(true) {
            return Some((m + rel, m + rel + 5));
        }
    }
    let rel = rest.find("true")?;
    Some((m + rel, m + rel + 4))
}

pub fn parse(text: &str) -> Option<WmConfig> {
    let mut cfg = WmConfig::default();

    // カーソル
    let (co, cc) = cursor_block_range(text)?;
    let cursor = &text[co..=cc];
    cfg.cursor_theme = raw_str_field(cursor, "theme")?;
    cfg.cursor_size = raw_num_field(cursor, "size")?;

    // 入力（全体）
    let (go, gc) = input_global_range(text)?;
    let global = &text[go..=gc];
    let (ko, kc) = sub_block_range(global, "keyboard")?;
    let kb = &global[ko..=kc];
    if !kb.contains("repeatRate") {
        return None;
    }
    cfg.kb_layout = raw_str_field(kb, "layout").unwrap_or_else(|| "us".to_string());
    cfg.kb_options = raw_str_field(kb, "options").unwrap_or_default();
    cfg.kb_repeat_rate = raw_num_field(kb, "repeatRate")?;
    cfg.kb_repeat_delay = raw_num_field(kb, "repeatDelay")?;

    if let Some((to, tc)) = sub_block_range(global, "touchpad") {
        let tp = &global[to..=tc];
        cfg.touchpad = Touchpad {
            present: true,
            tap_to_click: raw_bool_field(tp, "tapToClick").unwrap_or(true),
            natural_scroll: raw_bool_field(tp, "naturalScroll").unwrap_or(false),
            scroll_method: raw_str_field(tp, "scrollMethod")
                .unwrap_or_else(|| "twoFinger".to_string()),
            scroll_factor: raw_num_field(tp, "scrollFactor").unwrap_or(1.0),
            disable_while_typing: raw_bool_field(tp, "disableWhileTyping").unwrap_or(true),
            click_method: raw_str_field(tp, "clickMethod"),
            tap_button_map: raw_str_field(tp, "tapButtonMap"),
        };
    }
    if let Some((po, pc)) = sub_block_range(global, "pointer") {
        let pt = &global[po..=pc];
        cfg.pointer = Pointer {
            present: true,
            accel: raw_num_field(pt, "pointerAccel").unwrap_or(0.0),
            accel_profile: raw_str_field(pt, "accelProfile")
                .unwrap_or_else(|| "adaptive".to_string()),
            left_handed: raw_bool_field(pt, "leftHanded"),
            middle_emulation: raw_bool_field(pt, "middleEmulation"),
            natural_scroll: raw_bool_field(pt, "naturalScroll"),
        };
    }

    // 出力
    cfg.outputs = parse_outputs(text);

    // 描画
    if let Some((s, e)) = frame_pacing_range(text) {
        cfg.frame_pacing = text[s..e].to_string();
        cfg.frame_pacing_present = true;
    }
    if let Some((s, e)) = fps_range(text) {
        cfg.fps_initial = &text[s..e] == "true";
        cfg.fps_present = true;
    }

    // リッド
    if let Some((lo, lc)) = lid_block_range(text) {
        let lid = &text[lo..=lc];
        cfg.lid = Lid {
            on_battery: parse_lid_field(lid, "onBattery"),
            on_external: parse_lid_field(lid, "onExternalPower"),
            internal_panel_off: raw_bool_field(lid, "internalPanelOff").unwrap_or(true),
        };
    }

    // ホットコーナー
    cfg.hotcorners = parse_hotcorners(text);

    Some(cfg)
}

// ---------- index.tsx への反映 ----------

/// 値が変わっている項目だけを書き換える。想定外の形なら None（保存中止）。
/// 最後に読み直して意図した値と一致することを検証する。
pub fn apply(text: &str, cfg: &WmConfig) -> Option<String> {
    let old = parse(text)?;
    let mut out = text.to_string();

    // カーソル
    if cfg.cursor_theme != old.cursor_theme || cfg.cursor_size != old.cursor_size {
        let (co, cc) = cursor_block_range(&out)?;
        let mut block = out[co..=cc].to_string();
        if cfg.cursor_theme != old.cursor_theme {
            block = set_field(&block, "theme", Some(&raw_str(&cfg.cursor_theme)))?;
        }
        if cfg.cursor_size != old.cursor_size {
            // WM 側は整数（1..=512）しか受け付けない。小数で書くと起動エラーになる。
            block = set_field(&block, "size", Some(&raw_num(cfg.cursor_size.round())))?;
        }
        out.replace_range(co..=cc, &block);
    }

    // 入力（全体）
    let input_changed = cfg.kb_layout != old.kb_layout
        || cfg.kb_options != old.kb_options
        || cfg.kb_repeat_rate != old.kb_repeat_rate
        || cfg.kb_repeat_delay != old.kb_repeat_delay
        || cfg.touchpad != old.touchpad
        || cfg.pointer != old.pointer;
    if input_changed {
        let (go, gc) = input_global_range(&out)?;
        let mut global = out[go..=gc].to_string();

        // キーボード
        if cfg.kb_layout != old.kb_layout
            || cfg.kb_options != old.kb_options
            || cfg.kb_repeat_rate != old.kb_repeat_rate
            || cfg.kb_repeat_delay != old.kb_repeat_delay
        {
            let (ko, kc) = sub_block_range(&global, "keyboard")?;
            let mut kb = global[ko..=kc].to_string();
            if cfg.kb_layout != old.kb_layout {
                kb = set_field(&kb, "layout", Some(&raw_str(&cfg.kb_layout)))?;
            }
            if cfg.kb_options != old.kb_options {
                let v = if cfg.kb_options.is_empty() {
                    None
                } else {
                    Some(raw_str(&cfg.kb_options))
                };
                kb = set_field(&kb, "options", v.as_deref())?;
            }
            if cfg.kb_repeat_rate != old.kb_repeat_rate {
                kb = set_field(&kb, "repeatRate", Some(&raw_num(cfg.kb_repeat_rate.round())))?;
            }
            if cfg.kb_repeat_delay != old.kb_repeat_delay {
                kb = set_field(&kb, "repeatDelay", Some(&raw_num(cfg.kb_repeat_delay.round())))?;
            }
            global.replace_range(ko..=kc, &kb);
        }

        // タッチパッド
        if cfg.touchpad != old.touchpad {
            if cfg.touchpad.present {
                let (to, tc) = sub_block_range(&global, "touchpad")?;
                let mut tp = global[to..=tc].to_string();
                let n = &cfg.touchpad;
                let o = &old.touchpad;
                if n.tap_to_click != o.tap_to_click {
                    tp = set_field(&tp, "tapToClick", Some(&raw_bool(n.tap_to_click)))?;
                }
                if n.natural_scroll != o.natural_scroll {
                    tp = set_field(&tp, "naturalScroll", Some(&raw_bool(n.natural_scroll)))?;
                }
                if n.scroll_method != o.scroll_method {
                    tp = set_field(&tp, "scrollMethod", Some(&raw_str(&n.scroll_method)))?;
                }
                if n.scroll_factor != o.scroll_factor {
                    tp = set_field(&tp, "scrollFactor", Some(&raw_num(n.scroll_factor)))?;
                }
                if n.disable_while_typing != o.disable_while_typing {
                    tp = set_field(
                        &tp,
                        "disableWhileTyping",
                        Some(&raw_bool(n.disable_while_typing)),
                    )?;
                }
                if n.click_method != o.click_method {
                    let v = n.click_method.as_deref().map(raw_str);
                    tp = set_field(&tp, "clickMethod", v.as_deref())?;
                }
                if n.tap_button_map != o.tap_button_map {
                    let v = n.tap_button_map.as_deref().map(raw_str);
                    tp = set_field(&tp, "tapButtonMap", v.as_deref())?;
                }
                global.replace_range(to..=tc, &tp);
            } else {
                // ブロック自体が無い config へ、ブロックごと挿入する
                let n = &cfg.touchpad;
                let mut body = String::from("{\n");
                let ind = "      ";
                body.push_str(&format!("{ind}tapToClick: {},\n", raw_bool(n.tap_to_click)));
                body.push_str(&format!(
                    "{ind}naturalScroll: {},\n",
                    raw_bool(n.natural_scroll)
                ));
                body.push_str(&format!(
                    "{ind}scrollMethod: {},\n",
                    raw_str(&n.scroll_method)
                ));
                body.push_str(&format!(
                    "{ind}scrollFactor: {},\n",
                    raw_num(n.scroll_factor)
                ));
                body.push_str(&format!(
                    "{ind}disableWhileTyping: {},\n",
                    raw_bool(n.disable_while_typing)
                ));
                if let Some(v) = &n.click_method {
                    body.push_str(&format!("{ind}clickMethod: {},\n", raw_str(v)));
                }
                if let Some(v) = &n.tap_button_map {
                    body.push_str(&format!("{ind}tapButtonMap: {},\n", raw_str(v)));
                }
                body.push_str("    }");
                global = set_field(&global, "touchpad", Some(&body))?;
            }
        }

        // ポインター
        if cfg.pointer != old.pointer {
            if cfg.pointer.present {
                let (po, pc) = sub_block_range(&global, "pointer")?;
                let mut pt = global[po..=pc].to_string();
                let n = &cfg.pointer;
                let o = &old.pointer;
                if n.accel != o.accel {
                    pt = set_field(&pt, "pointerAccel", Some(&raw_num(n.accel)))?;
                }
                if n.accel_profile != o.accel_profile {
                    pt = set_field(&pt, "accelProfile", Some(&raw_str(&n.accel_profile)))?;
                }
                if n.left_handed != o.left_handed {
                    let v = n.left_handed.map(raw_bool);
                    pt = set_field(&pt, "leftHanded", v.as_deref())?;
                }
                if n.middle_emulation != o.middle_emulation {
                    let v = n.middle_emulation.map(raw_bool);
                    pt = set_field(&pt, "middleEmulation", v.as_deref())?;
                }
                if n.natural_scroll != o.natural_scroll {
                    let v = n.natural_scroll.map(raw_bool);
                    pt = set_field(&pt, "naturalScroll", v.as_deref())?;
                }
                global.replace_range(po..=pc, &pt);
            } else {
                let n = &cfg.pointer;
                let mut body = String::from("{\n");
                let ind = "      ";
                body.push_str(&format!("{ind}pointerAccel: {},\n", raw_num(n.accel)));
                body.push_str(&format!(
                    "{ind}accelProfile: {},\n",
                    raw_str(&n.accel_profile)
                ));
                if let Some(v) = n.left_handed {
                    body.push_str(&format!("{ind}leftHanded: {},\n", raw_bool(v)));
                }
                if let Some(v) = n.middle_emulation {
                    body.push_str(&format!("{ind}middleEmulation: {},\n", raw_bool(v)));
                }
                if let Some(v) = n.natural_scroll {
                    body.push_str(&format!("{ind}naturalScroll: {},\n", raw_bool(v)));
                }
                body.push_str("    }");
                global = set_field(&global, "pointer", Some(&body))?;
            }
        }

        out.replace_range(go..=gc, &global);
    }

    // 出力
    if cfg.outputs != old.outputs {
        if cfg.outputs.len() != old.outputs.len() {
            return None;
        }
        for (n, o) in cfg.outputs.iter().zip(old.outputs.iter()) {
            if n.name != o.name {
                return None;
            }
            if n == o {
                continue;
            }
            // 名前で対象エントリを探し直す（前の編集で位置がずれているため）
            let needle = format!("display[\"{}\"]", n.name);
            let at = out.find(&needle)?;
            let (bo, bc) = block_range(&out, at)?;
            let mut block = out[bo..=bc].to_string();
            if n.scale != o.scale {
                block = set_field(&block, "scale", Some(&raw_num(n.scale)))?;
            }
            if n.vrr != o.vrr {
                let v = n.vrr.map(raw_bool);
                block = set_field(&block, "vrr", v.as_deref())?;
            }
            if n.hdr != o.hdr {
                let v = match n.hdr {
                    Hdr::Unset => None,
                    Hdr::Off => Some("false".to_string()),
                    Hdr::On => Some("true".to_string()),
                    Hdr::Options => return None, // 詳細形式へは書き換えない
                };
                block = set_field(&block, "hdr", v.as_deref())?;
            }
            if n.direct_scanout != o.direct_scanout {
                let v = n.direct_scanout.map(raw_bool);
                block = set_field(&block, "directScanout", v.as_deref())?;
            }
            if n.subpixel != o.subpixel {
                let v = n.subpixel.as_deref().map(raw_str);
                block = set_field(&block, "subpixel", v.as_deref())?;
            }
            if n.icc != o.icc {
                let v = n.icc.as_deref().map(raw_str);
                block = set_field(&block, "icc", v.as_deref())?;
            }
            out.replace_range(bo..=bc, &block);
        }
    }

    // フレームペーシング
    if cfg.frame_pacing != old.frame_pacing {
        if !old.frame_pacing_present {
            return None;
        }
        let (s, e) = frame_pacing_range(&out)?;
        out.replace_range(s..e, &cfg.frame_pacing);
    }

    // FPS カウンターの初期表示
    if cfg.fps_initial != old.fps_initial {
        if !old.fps_present {
            return None;
        }
        let (s, e) = fps_range(&out)?;
        out.replace_range(s..e, if cfg.fps_initial { "true" } else { "false" });
    }

    // リッド
    if cfg.lid != old.lid {
        let (lo, lc) = lid_block_range(&out)?;
        let mut lid = out[lo..=lc].to_string();
        let n = &cfg.lid;
        let o = &old.lid;
        if n.internal_panel_off != o.internal_panel_off {
            lid = set_field(
                &lid,
                "internalPanelOff",
                Some(&raw_bool(n.internal_panel_off)),
            )?;
        }
        for (key, nv, ov) in [
            ("onBattery", &n.on_battery, &o.on_battery),
            ("onExternalPower", &n.on_external, &o.on_external),
        ] {
            if nv != ov {
                match nv {
                    LidField::Value(v) => {
                        lid = set_field(&lid, key, Some(&raw_str(v)))?;
                    }
                    LidField::Absent => {
                        lid = set_field(&lid, key, None)?;
                    }
                    LidField::Function => return None,
                }
            }
        }
        out.replace_range(lo..=lc, &lid);
    }

    // ホットコーナー
    if cfg.hotcorners != old.hotcorners {
        let (Some(new_corners), Some(_)) = (cfg.hotcorners, old.hotcorners) else {
            return None;
        };
        let (s, e) = hot_region(&out)?;
        let mut body = String::new();
        for (idx, key) in CORNER_KEYS.iter().enumerate() {
            if let Some(line) = hotcorner_line(key, new_corners[idx]) {
                body.push_str(&line);
                body.push('\n');
            }
        }
        out.replace_range(s..e, &body);
    }

    // 検証：書き換えた結果を読み直して、意図した値と一致すること。
    if parse(&out)? != *cfg {
        return None;
    }
    Some(out)
}

// ---------- autostart.ts ----------

const AUTOSTART_HEADER: &str = "import { COMPOSITOR } from \"shoji_wm\";\n\n// このファイルは「ShojiWM 設定」アプリが管理しています。\n// 起動コマンドの追加・変更は、設定アプリの「起動」ページから行ってください。\n// 手で編集しても、アプリで保存すると上書きされます。\n\n";

const AUTOSTART_FOOTER: &str = "\nexport function installAutostart(): void {\n  for (const entry of ENTRIES) {\n    if (entry.service) {\n      COMPOSITOR.process.service(entry.name, {\n        command: entry.command,\n        restart: \"on-exit\",\n      });\n    } else {\n      COMPOSITOR.process.once(entry.name, {\n        command: entry.command,\n        runPolicy: \"once-per-session\",\n      });\n    }\n  }\n}\n";

/// autostart.ts の中身を生成する。
pub fn generate_autostart(entries: &[AutoEntry]) -> String {
    let mut out = String::from(AUTOSTART_HEADER);
    out.push_str("const ENTRIES: { name: string; command: string; service: boolean }[] = [");
    if entries.is_empty() {
        out.push_str("];\n");
    } else {
        out.push('\n');
        for e in entries {
            out.push_str(&format!(
                "  {{ name: {}, command: {}, service: {} }},\n",
                serde_json::to_string(&e.name).unwrap_or_else(|_| "\"\"".to_string()),
                serde_json::to_string(&e.command).unwrap_or_else(|_| "\"\"".to_string()),
                if e.service { "true" } else { "false" },
            ));
        }
        out.push_str("];\n");
    }
    out.push_str(AUTOSTART_FOOTER);
    out
}

fn json_str_field(block: &str, key: &str) -> Option<String> {
    let raw = field_raw(block, key)?;
    serde_json::from_str::<String>(raw.trim()).ok()
}

/// autostart.ts を読む。ENTRIES の形が想定と違えば None。
pub fn parse_autostart(text: &str) -> Option<Vec<AutoEntry>> {
    let m = text.find("const ENTRIES")?;
    // 型注釈の `[]` ではなく、代入側の `[` を探す
    let eq = text[m..].find("= [")? + m;
    let (ao, ac) = pair_range(text, eq, b'[', b']')?;
    let arr = &text[ao..=ac];
    let mut entries = Vec::new();
    let mut pos = 0;
    while let Some((o, c)) = block_range(arr, pos) {
        let obj = &arr[o..=c];
        entries.push(AutoEntry {
            name: json_str_field(obj, "name")?,
            command: json_str_field(obj, "command")?,
            service: raw_bool_field(obj, "service").unwrap_or(false),
        });
        pos = c + 1;
    }
    Some(entries)
}

/// autostart.ts を保存する（既存があればバックアップしてから）。
pub fn save_autostart(entries: &[AutoEntry]) -> std::io::Result<()> {
    let path = autostart_path();
    if path.exists() {
        let (stamp, _) = jst_stamp(now_epoch());
        let backup = path.with_file_name(format!("autostart.ts.bak-settings-{stamp}"));
        fs::copy(&path, &backup)?;
    }
    let tmp = path.with_file_name("autostart.ts.tmp-settings");
    fs::write(&tmp, generate_autostart(entries))?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

// ---------- バックアップ ----------

/// JST（UTC+9固定）での `YYYYMMDD-HHMMSS` と表示用 `YYYY/MM/DD HH:MM` を返す。
fn jst_stamp(epoch_secs: u64) -> (String, String) {
    let t = epoch_secs + 9 * 3600;
    let days = (t / 86400) as i64;
    let secs = t % 86400;
    let (h, mi, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    // civil-from-days (Howard Hinnant のアルゴリズム)
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (
        format!("{y:04}{m:02}{d:02}-{h:02}{mi:02}{s:02}"),
        format!("{y:04}/{m:02}/{d:02} {h:02}:{mi:02}"),
    )
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// バックアップファイル名から表示名を作る（対象ファイル名＋日時）。
pub fn backup_display_name(path: &PathBuf) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    for target in ["index.tsx", "autostart.ts"] {
        let prefix = format!("{target}.bak-settings-");
        if let Some(stamp) = name.strip_prefix(&prefix) {
            let b = stamp.as_bytes();
            if b.len() == 15 && b[8] == b'-' {
                return format!(
                    "{target} · {}/{}/{} {}:{}",
                    &stamp[0..4],
                    &stamp[4..6],
                    &stamp[6..8],
                    &stamp[9..11],
                    &stamp[11..13]
                );
            }
        }
    }
    name
}

fn make_backup() -> std::io::Result<PathBuf> {
    let path = config_path();
    let (stamp, _) = jst_stamp(now_epoch());
    let backup = path.with_file_name(format!("index.tsx.bak-settings-{stamp}"));
    fs::copy(&path, &backup)?;
    Ok(backup)
}

/// バックアップを作ってから、新しいテキストで設定ファイルを置き換える。
pub fn save(new_text: &str) -> std::io::Result<PathBuf> {
    let path = config_path();
    let backup = make_backup()?;
    let tmp = path.with_file_name("index.tsx.tmp-settings");
    fs::write(&tmp, new_text)?;
    fs::rename(&tmp, &path)?;
    Ok(backup)
}

/// このアプリが作ったバックアップの一覧（新しい順、index.tsx と autostart.ts の両方）。
pub fn list_backups() -> Vec<PathBuf> {
    let dir = config_dir();
    let mut items: Vec<PathBuf> = fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name().map(|n| {
                        let n = n.to_string_lossy();
                        n.starts_with("index.tsx.bak-settings-")
                            || n.starts_with("autostart.ts.bak-settings-")
                    }).unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    items.sort();
    items.reverse();
    items
}

/// バックアップへ復元する。対象ファイルの今の状態も先にバックアップする。
pub fn restore(backup: &PathBuf) -> std::io::Result<()> {
    let name = backup
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name.starts_with("autostart.ts.") {
        let path = autostart_path();
        if path.exists() {
            let (stamp, _) = jst_stamp(now_epoch());
            let cur = path.with_file_name(format!("autostart.ts.bak-settings-{stamp}"));
            fs::copy(&path, &cur)?;
        }
        fs::copy(backup, &path)?;
    } else {
        let path = config_path();
        let _ = make_backup();
        fs::copy(backup, &path)?;
    }
    Ok(())
}

// ---------- バー（shoji-bar-4）の配色に合わせる ----------

pub struct BarTheme {
    pub seed: u32,
    pub palette: String,
    pub dark: bool,
}

impl Default for BarTheme {
    fn default() -> Self {
        Self {
            seed: 0x6750a4,
            palette: "tonal-spot".to_string(),
            dark: true,
        }
    }
}

/// バーの設定 `$XDG_STATE_HOME/shoji-bar-4/settings.json` を読み、今のシード色・
/// パレット・ダーク可否を返す。読めなければ Material 基準の既定値。
pub fn load_bar_theme() -> BarTheme {
    let mut theme = BarTheme::default();
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let path = state.join("shoji-bar-4").join("settings.json");
    let Ok(text) = fs::read_to_string(&path) else {
        return theme;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return theme;
    };
    if let Some(seed) = v.get("seed").and_then(|s| s.as_str()) {
        if let Some(hex) = seed.strip_prefix('#') {
            if let Ok(n) = u32::from_str_radix(hex, 16) {
                theme.seed = n;
            }
        }
    }
    if let Some(p) = v.get("palette").and_then(|s| s.as_str()) {
        theme.palette = p.to_string();
    }
    if let Some(mode) = v.get("mode").and_then(|s| s.as_str()) {
        theme.dark = mode != "light";
    }
    theme
}

/// インストール済みカーソルテーマ名（cursors フォルダを持つもの）。
pub fn installed_cursor_themes() -> Vec<String> {
    let mut dirs = vec![PathBuf::from("/usr/share/icons")];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        dirs.push(home.join(".local/share/icons"));
        dirs.push(home.join(".icons"));
    }
    let mut names: Vec<String> = Vec::new();
    for dir in dirs {
        if let Ok(rd) = fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.join("cursors").is_dir() {
                    if let Some(n) = p.file_name().map(|n| n.to_string_lossy().into_owned()) {
                        if !names.contains(&n) {
                            names.push(n);
                        }
                    }
                }
            }
        }
    }
    names
}

/// ShojiWM 本体の commit（~/ShojiWM の .git から）。取れなければ None。
pub fn shojiwm_commit() -> Option<String> {
    let home = std::env::var_os("HOME")?;
    let git = PathBuf::from(home).join("ShojiWM").join(".git");
    let head = fs::read_to_string(git.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(r) = head.strip_prefix("ref: ") {
        let hash = fs::read_to_string(git.join(r)).ok()?;
        Some(hash.trim().chars().take(7).collect())
    } else {
        Some(head.chars().take(7).collect())
    }
}

/// index.tsx に組み込み（autostart の import と呼び出し、ホットコーナーの領域）が
/// 済んでいるかを返す。アプリの「について」や確認モードでの表示用。
pub fn wiring_status(text: &str) -> (bool, bool) {
    let autostart = text.contains("installAutostart");
    let hotcorners = text.contains(HOT_BEGIN) && text.contains(HOT_END);
    (autostart, hotcorners)
}
