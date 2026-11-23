//! 画面。ページは「カーソル／キーボード／ポインター／画面／起動／電源／
//! バックアップ／について」の8つ。
//! 保存は変更があるときだけ押せる状態設計にし、結果はスナックバーで伝える。

use anti_webui::prelude::*;
use anti_webui_md3e::color_scheme;
use anti_webui_md3e::components::{
    ButtonStyle, MenuStyle, SliderStyle, Snackbar, TextFieldStyle, dialog, filled_card, icon,
    menu_item, nav_item, navigation_rail, sized_icon, snackbars, styled_button, styled_menu,
    styled_slider, switch, text_field,
};
use anti_webui_md3e::icons;
use anti_webui_md3e::typography::TextStyle;

use crate::config::{
    self, AutoEntry, CornerAction, Corners, Hdr, LidField, OutputCfg, WmConfig, CORNER_LABELS,
};
use std::path::PathBuf;

// ---------- フォーム状態 ----------

#[derive(Clone)]
struct OutputForm {
    scale: Signal<f32>,
    vrr: Signal<bool>,
    hdr_on: Signal<bool>,
    scanout: Signal<bool>,
    subpixel: Signal<String>,
    icc: Signal<String>,
    orig: OutputCfg,
}

impl OutputForm {
    fn new(o: &OutputCfg) -> Self {
        Self {
            scale: signal(o.scale),
            vrr: signal(o.vrr.unwrap_or(false)),
            hdr_on: signal(o.hdr.is_on()),
            scanout: signal(o.direct_scanout.unwrap_or(true)),
            subpixel: signal(o.subpixel.clone().unwrap_or_default()),
            icc: signal(o.icc.clone().unwrap_or_default()),
            orig: o.clone(),
        }
    }
    fn value(&self) -> OutputCfg {
        let o = &self.orig;
        OutputCfg {
            name: o.name.clone(),
            scale: self.scale.get(),
            vrr: if self.vrr.get() { Some(true) } else { None },
            hdr: {
                let on = self.hdr_on.get();
                if on == o.hdr.is_on() {
                    o.hdr
                } else if on {
                    Hdr::On
                } else {
                    Hdr::Off
                }
            },
            direct_scanout: {
                let b = self.scanout.get();
                if b == o.direct_scanout.unwrap_or(true) {
                    o.direct_scanout
                } else {
                    Some(b)
                }
            },
            subpixel: {
                let s = self.subpixel.with(|s| s.clone());
                if s.is_empty() { None } else { Some(s) }
            },
            icc: {
                let s = self.icc.with(|s| s.clone());
                if s.is_empty() { None } else { Some(s) }
            },
        }
    }
}

#[derive(Clone)]
struct Form {
    cursor_theme: Signal<String>,
    cursor_size: Signal<f32>,
    kb_layout: Signal<String>,
    kb_options: Signal<String>,
    kb_rate: Signal<f32>,
    kb_delay: Signal<f32>,
    tp_tap: Signal<bool>,
    tp_natural: Signal<bool>,
    tp_method: Signal<String>,
    tp_factor: Signal<f32>,
    tp_dwt: Signal<bool>,
    tp_click: Signal<String>,
    tp_tapmap: Signal<String>,
    pt_accel: Signal<f32>,
    pt_profile: Signal<String>,
    pt_left: Signal<String>,
    pt_middle: Signal<String>,
    pt_natural: Signal<String>,
    outputs: Vec<OutputForm>,
    pacing: Signal<String>,
    fps: Signal<bool>,
    lid_battery: Signal<String>,
    lid_external: Signal<String>,
    lid_panel_off: Signal<bool>,
    lid_orig: (LidField, LidField),
    corners: [Signal<String>; 4],
    corners_supported: bool,
    entries: Signal<Vec<AutoEntry>>,
}

fn tri_of(v: Option<bool>) -> Signal<String> {
    signal(match v {
        None => "unset".to_string(),
        Some(true) => "true".to_string(),
        Some(false) => "false".to_string(),
    })
}

fn tri_value(sig: &Signal<String>) -> Option<bool> {
    match sig.with(|s| s.clone()).as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn corner_key_of(a: CornerAction) -> &'static str {
    match a {
        CornerAction::None => "none",
        CornerAction::Switcher => "switcher",
        CornerAction::WorkspaceNext => "next",
        CornerAction::WorkspacePrev => "prev",
        CornerAction::Lock => "lock",
    }
}

fn corner_action_of(key: &str) -> CornerAction {
    match key {
        "switcher" => CornerAction::Switcher,
        "next" => CornerAction::WorkspaceNext,
        "prev" => CornerAction::WorkspacePrev,
        "lock" => CornerAction::Lock,
        _ => CornerAction::None,
    }
}

fn lid_key_of(f: &LidField) -> String {
    match f {
        LidField::Absent => String::new(),
        LidField::Value(v) => v.clone(),
        LidField::Function => String::new(),
    }
}

impl Form {
    fn new(cfg: &WmConfig, entries: Vec<AutoEntry>) -> Self {
        let corners: [Signal<String>; 4] = match cfg.hotcorners {
            Some(c) => c.map(|a| signal(corner_key_of(a).to_string())),
            None => [
                signal("none".to_string()),
                signal("none".to_string()),
                signal("none".to_string()),
                signal("none".to_string()),
            ],
        };
        Self {
            cursor_theme: signal(cfg.cursor_theme.clone()),
            cursor_size: signal(cfg.cursor_size),
            kb_layout: signal(cfg.kb_layout.clone()),
            kb_options: signal(cfg.kb_options.clone()),
            kb_rate: signal(cfg.kb_repeat_rate),
            kb_delay: signal(cfg.kb_repeat_delay),
            tp_tap: signal(cfg.touchpad.tap_to_click),
            tp_natural: signal(cfg.touchpad.natural_scroll),
            tp_method: signal(cfg.touchpad.scroll_method.clone()),
            tp_factor: signal(cfg.touchpad.scroll_factor),
            tp_dwt: signal(cfg.touchpad.disable_while_typing),
            tp_click: signal(cfg.touchpad.click_method.clone().unwrap_or_default()),
            tp_tapmap: signal(cfg.touchpad.tap_button_map.clone().unwrap_or_default()),
            pt_accel: signal(cfg.pointer.accel),
            pt_profile: signal(cfg.pointer.accel_profile.clone()),
            pt_left: tri_of(cfg.pointer.left_handed),
            pt_middle: tri_of(cfg.pointer.middle_emulation),
            pt_natural: tri_of(cfg.pointer.natural_scroll),
            outputs: cfg.outputs.iter().map(OutputForm::new).collect(),
            pacing: signal(cfg.frame_pacing.clone()),
            fps: signal(cfg.fps_initial),
            lid_battery: signal(lid_key_of(&cfg.lid.on_battery)),
            lid_external: signal(lid_key_of(&cfg.lid.on_external)),
            lid_panel_off: signal(cfg.lid.internal_panel_off),
            lid_orig: (cfg.lid.on_battery.clone(), cfg.lid.on_external.clone()),
            corners,
            corners_supported: cfg.hotcorners.is_some(),
            entries: signal(entries),
        }
    }

    fn values(&self, base: &WmConfig) -> WmConfig {
        let mut cfg = base.clone();
        cfg.cursor_theme = self.cursor_theme.with(|s| s.clone());
        cfg.cursor_size = self.cursor_size.get();
        cfg.kb_layout = self.kb_layout.with(|s| s.clone());
        cfg.kb_options = self.kb_options.with(|s| s.clone());
        cfg.kb_repeat_rate = self.kb_rate.get();
        cfg.kb_repeat_delay = self.kb_delay.get();
        cfg.touchpad.tap_to_click = self.tp_tap.get();
        cfg.touchpad.natural_scroll = self.tp_natural.get();
        cfg.touchpad.scroll_method = self.tp_method.with(|s| s.clone());
        cfg.touchpad.scroll_factor = self.tp_factor.get();
        cfg.touchpad.disable_while_typing = self.tp_dwt.get();
        cfg.touchpad.click_method = {
            let s = self.tp_click.with(|s| s.clone());
            if s.is_empty() { None } else { Some(s) }
        };
        cfg.touchpad.tap_button_map = {
            let s = self.tp_tapmap.with(|s| s.clone());
            if s.is_empty() { None } else { Some(s) }
        };
        cfg.pointer.accel = self.pt_accel.get();
        cfg.pointer.accel_profile = self.pt_profile.with(|s| s.clone());
        cfg.pointer.left_handed = tri_value(&self.pt_left);
        cfg.pointer.middle_emulation = tri_value(&self.pt_middle);
        cfg.pointer.natural_scroll = tri_value(&self.pt_natural);
        cfg.outputs = self.outputs.iter().map(|o| o.value()).collect();
        cfg.frame_pacing = self.pacing.with(|s| s.clone());
        cfg.fps_initial = self.fps.get();
        let lid_from = |sig: &Signal<String>, orig: &LidField| -> LidField {
            if matches!(orig, LidField::Function) {
                return orig.clone();
            }
            let s = sig.with(|s| s.clone());
            if s.is_empty() {
                LidField::Absent
            } else {
                LidField::Value(s)
            }
        };
        cfg.lid.on_battery = lid_from(&self.lid_battery, &self.lid_orig.0);
        cfg.lid.on_external = lid_from(&self.lid_external, &self.lid_orig.1);
        cfg.lid.internal_panel_off = self.lid_panel_off.get();
        if self.corners_supported {
            let mut c: Corners = [CornerAction::None; 4];
            for i in 0..4 {
                c[i] = corner_action_of(&self.corners[i].with(|s| s.clone()));
            }
            cfg.hotcorners = Some(c);
        }
        cfg
    }

    fn entries(&self) -> Vec<AutoEntry> {
        self.entries.with(|v| v.clone())
    }

    /// 復元後に、既存のシグナルへ新しい値を流し込む（ページ側の束縛を保つため）。
    fn load(&mut self, cfg: &WmConfig, entries: Vec<AutoEntry>) {
        let fresh = Form::new(cfg, entries);
        self.cursor_theme.set(fresh.cursor_theme.with(|s| s.clone()));
        self.cursor_size.set(fresh.cursor_size.get());
        self.kb_layout.set(fresh.kb_layout.with(|s| s.clone()));
        self.kb_options.set(fresh.kb_options.with(|s| s.clone()));
        self.kb_rate.set(fresh.kb_rate.get());
        self.kb_delay.set(fresh.kb_delay.get());
        self.tp_tap.set(fresh.tp_tap.get());
        self.tp_natural.set(fresh.tp_natural.get());
        self.tp_method.set(fresh.tp_method.with(|s| s.clone()));
        self.tp_factor.set(fresh.tp_factor.get());
        self.tp_dwt.set(fresh.tp_dwt.get());
        self.tp_click.set(fresh.tp_click.with(|s| s.clone()));
        self.tp_tapmap.set(fresh.tp_tapmap.with(|s| s.clone()));
        self.pt_accel.set(fresh.pt_accel.get());
        self.pt_profile.set(fresh.pt_profile.with(|s| s.clone()));
        self.pt_left.set(fresh.pt_left.with(|s| s.clone()));
        self.pt_middle.set(fresh.pt_middle.with(|s| s.clone()));
        self.pt_natural.set(fresh.pt_natural.with(|s| s.clone()));
        if self.outputs.len() == fresh.outputs.len() {
            for (mine, new) in self.outputs.iter_mut().zip(fresh.outputs.iter()) {
                mine.scale.set(new.scale.get());
                mine.vrr.set(new.vrr.get());
                mine.hdr_on.set(new.hdr_on.get());
                mine.scanout.set(new.scanout.get());
                mine.subpixel.set(new.subpixel.with(|s| s.clone()));
                mine.icc.set(new.icc.with(|s| s.clone()));
                mine.orig = new.orig.clone();
            }
        } else {
            self.outputs = fresh.outputs.clone();
        }
        self.pacing.set(fresh.pacing.with(|s| s.clone()));
        self.fps.set(fresh.fps.get());
        self.lid_battery.set(fresh.lid_battery.with(|s| s.clone()));
        self.lid_external.set(fresh.lid_external.with(|s| s.clone()));
        self.lid_panel_off.set(fresh.lid_panel_off.get());
        self.lid_orig = fresh.lid_orig.clone();
        for i in 0..4 {
            self.corners[i].set(fresh.corners[i].with(|s| s.clone()));
        }
        self.corners_supported = fresh.corners_supported;
        self.entries.set(fresh.entries());
    }
}

// ---------- 小物 ----------

fn txt(style: TextStyle, f: impl Fn() -> String + 'static) -> Div {
    style.apply(div().whitespace_nowrap().child(text(f)))
}

fn static_txt(style: TextStyle, s: impl Into<String>) -> Div {
    style.apply(div().whitespace_nowrap().child(s.into()))
}

fn muted(style: TextStyle, s: impl Into<String>) -> Div {
    static_txt(style, s).dyn_style(|s| s.text_color(color_scheme().on_surface_variant))
}

fn section_card(icon_view: Div, title: &'static str, body: impl IntoView) -> Div {
    filled_card()
        .flex()
        .flex_col()
        .gap(16.0)
        .p(20.0)
        .w_full()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(8.0)
                .child(icon_view)
                .child(static_txt(TextStyle::TITLE_MEDIUM, title)),
        )
        .child(body)
}

fn page_shell(title: &'static str, body: impl IntoView) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(16.0)
        .p(24.0)
        .w_full()
        .child(static_txt(TextStyle::HEADLINE_SMALL, title))
        .child(body)
}

fn slider_row(label: &'static str, value: Signal<f32>, range: std::ops::RangeInclusive<f32>) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(12.0)
        .w_full()
        .child(div().w(120.0).flex_none().child(static_txt(TextStyle::TITLE_SMALL, label)))
        .child(styled_slider(value, SliderStyle::new().range(range)).flex_1().min_w(0.0))
        .child(
            div()
                .w(48.0)
                .flex_none()
                .child(txt(TextStyle::TITLE_SMALL, move || {
                    format!("{:.0}", value.get())
                })),
        )
}

fn slider_row_2dp(label: &'static str, value: Signal<f32>, range: std::ops::RangeInclusive<f32>) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(12.0)
        .w_full()
        .child(div().w(120.0).flex_none().child(static_txt(TextStyle::TITLE_SMALL, label)))
        .child(styled_slider(value, SliderStyle::new().range(range)).flex_1().min_w(0.0))
        .child(
            div()
                .w(48.0)
                .flex_none()
                .child(txt(TextStyle::TITLE_SMALL, move || {
                    format!("{:.2}", value.get())
                })),
        )
}

fn switch_row(label: &'static str, on: Signal<bool>) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(12.0)
        .w_full()
        .child(
            div()
                .flex_1()
                .min_w(0.0)
                .child(static_txt(TextStyle::TITLE_SMALL, label)),
        )
        .child(switch(on))
}

/// 選択チップの列。選ばれているものは filled、他は tonal で描き直す。
fn chips_row(label: &'static str, sig: Signal<String>, options: Vec<(&'static str, &'static str)>) -> Div {
    let chips = dyn_view(move || {
        let current = sig.with(|s| s.clone());
        let mut row = div().flex().flex_row().gap(8.0);
        for (label, value) in &options {
            let v = value.to_string();
            let style = if current == *value {
                ButtonStyle::filled()
            } else {
                ButtonStyle::tonal()
            };
            row = row.child(styled_button(*label, style).on_click(move |_| sig.set(v.clone())));
        }
        row
    });
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(12.0)
        .w_full()
        .child(div().w(120.0).flex_none().child(static_txt(TextStyle::TITLE_SMALL, label)))
        .child(chips)
}

fn tri_options() -> Vec<(&'static str, &'static str)> {
    vec![("既定", "unset"), ("オン", "true"), ("オフ", "false")]
}

/// ドロップダウン（md3e の menu）。選択肢が多めの項目用。
fn dropdown_row(label: &'static str, sig: Signal<String>, options: Vec<(&'static str, &'static str)>) -> Div {
    let open = signal(false);
    let anchor = node_ref();
    let options_for_trigger = options.clone();
    let trigger = dyn_view(move || {
        let current = sig.with(|s| s.clone());
        let name = options_for_trigger
            .iter()
            .find(|(_, v)| *v == current)
            .map(|(l, _)| *l)
            .unwrap_or("");
        styled_button(format!("{name}"), ButtonStyle::tonal().icon(icons::ARROW_DROP_DOWN))
            .on_click(move |_| open.set(true))
            .node_ref(anchor)
    });
    let items: Vec<anti_webui_md3e::components::MenuItem> = options
        .into_iter()
        .map(|(l, v)| {
            let value = v.to_string();
            let value_for_selected = value.clone();
            menu_item(l)
                .selected_when(move || sig.with(|s| s == &value_for_selected))
                .on_click(move || {
                    sig.set(value.clone());
                    open.set(false);
                })
        })
        .collect();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(12.0)
        .w_full()
        .child(div().w(120.0).flex_none().child(static_txt(TextStyle::TITLE_SMALL, label)))
        .child(trigger)
        .child(styled_menu(
            move || open.get(),
            move || open.set(false),
            vec![items],
            MenuStyle::new().anchor(anchor).match_anchor_width(),
        ))
}

// ---------- ページ ----------

fn cursor_page(form: &Form, themes: Vec<String>) -> Div {
    let theme_signal = form.cursor_theme;
    let themes_for_check = themes.clone();
    let preset_row = {
        let mut row = div().flex().flex_row().gap(8.0);
        for name in ["breeze_cursors", "Bibata-Modern-Ice", "Adwaita"] {
            let n = name.to_string();
            row = row.child(
                styled_button(name, ButtonStyle::tonal())
                    .on_click(move |_| theme_signal.set(n.clone())),
            );
        }
        row
    };
    let body = div()
        .flex()
        .flex_col()
        .gap(16.0)
        .w_full()
        .child(
            text_field(
                theme_signal,
                TextFieldStyle::new()
                    .label("テーマ")
                    .error_when(move || {
                        theme_signal.with(|s| !themes_for_check.contains(s))
                    }),
            )
            .w_full(),
        )
        .child(preset_row)
        .child(muted(
            TextStyle::BODY_SMALL,
            "存在しないテーマ名だと、ポインターが表示されなくなります",
        ))
        .child(slider_row("サイズ", form.cursor_size, 16.0..=48.0));
    let size_sig = form.cursor_size;
    let preview = dyn_view(move || {
        let size = size_sig.get();
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(12.0)
            .w_full()
            .p(16.0)
            .child(sized_icon(icons::ARROW_SELECTOR_TOOL, size))
            .child(muted(
                TextStyle::BODY_SMALL,
                format!("{size:.0}px での見え方（実寸）"),
            ))
    });
    page_shell(
        "カーソル",
        div()
            .flex()
            .flex_col()
            .gap(16.0)
            .w_full()
            .child(section_card(icon(icons::ARROW_SELECTOR_TOOL), "ポインター", body))
            .child(section_card(icon(icons::PREVIEW), "プレビュー", preview)),
    )
}

fn keyboard_page(form: &Form) -> Div {
    let layout_signal = form.kb_layout;
    let chips = div()
        .flex()
        .flex_row()
        .gap(8.0)
        .child(
            styled_button("jp", ButtonStyle::tonal())
                .on_click(move |_| layout_signal.set("jp".to_string())),
        )
        .child(
            styled_button("us", ButtonStyle::tonal())
                .on_click(move |_| layout_signal.set("us".to_string())),
        );
    let body = div()
        .flex()
        .flex_col()
        .gap(16.0)
        .w_full()
        .child(text_field(layout_signal, TextFieldStyle::new().label("レイアウト")).w_full())
        .child(chips)
        .child(
            text_field(form.kb_options, TextFieldStyle::new().label("オプション"))
                .w_full(),
        )
        .child(muted(
            TextStyle::BODY_SMALL,
            "例：caps:ctrl_modifier（Caps Lock を Ctrl にする）。空欄で無し",
        ))
        .child(slider_row("リピート速度", form.kb_rate, 10.0..=100.0))
        .child(slider_row("リピート遅延", form.kb_delay, 100.0..=1000.0));
    page_shell(
        "キーボード",
        section_card(icon(icons::KEYBOARD), "全体のキーボード設定", body),
    )
}

fn pointer_page(form: &Form) -> Div {
    let tp = div()
        .flex()
        .flex_col()
        .gap(12.0)
        .w_full()
        .child(switch_row("タップでクリック", form.tp_tap))
        .child(switch_row("ナチュラルスクロール", form.tp_natural))
        .child(switch_row("入力中は無効にする", form.tp_dwt))
        .child(dropdown_row(
            "スクロール方式",
            form.tp_method,
            vec![
                ("2本指", "twoFinger"),
                ("エッジ", "edge"),
                ("ボタン押下中", "onButtonDown"),
                ("なし", "none"),
            ],
        ))
        .child(dropdown_row(
            "クリック方式",
            form.tp_click,
            vec![
                ("既定", ""),
                ("ボタン領域", "buttonAreas"),
                ("指の本数", "clickfinger"),
            ],
        ))
        .child(dropdown_row(
            "タップの割当",
            form.tp_tapmap,
            vec![
                ("既定", ""),
                ("左・右・中", "leftRightMiddle"),
                ("左・中・右", "leftMiddleRight"),
            ],
        ))
        .child(slider_row_2dp("スクロール量", form.tp_factor, 0.1..=1.0));
    let pt = div()
        .flex()
        .flex_col()
        .gap(12.0)
        .w_full()
        .child(slider_row_2dp("加速度", form.pt_accel, -1.0..=1.0))
        .child(chips_row(
            "加速度の型",
            form.pt_profile,
            vec![("なめらか", "adaptive"), ("一定", "flat")],
        ))
        .child(chips_row("左右反転", form.pt_left, tri_options()))
        .child(chips_row("中ボタン代用", form.pt_middle, tri_options()))
        .child(chips_row("ナチュラルスクロール", form.pt_natural, tri_options()));
    page_shell(
        "ポインター",
        div()
            .flex()
            .flex_col()
            .gap(16.0)
            .w_full()
            .child(section_card(icon(icons::GESTURE), "タッチパッド", tp))
            .child(section_card(icon(icons::MOUSE), "マウス", pt)),
    )
}

fn corners_page(form: &Form) -> Div {
    let actions: [(&str, &str); 5] = [
        ("none", "なし"),
        ("switcher", "ウィンドウ切替"),
        ("next", "次のワークスペース"),
        ("prev", "前のワークスペース"),
        ("lock", "画面ロック"),
    ];
    let mut col = div().flex().flex_col().gap(12.0).w_full();
    if !form.corners_supported {
        col = col.child(muted(
            TextStyle::BODY_SMALL,
            "この設定ファイルにはホットコーナーの管理領域が無いため、編集できません",
        ));
    }
    for i in 0..4 {
        let sig = form.corners[i];
        let label = CORNER_LABELS[i];
        let options: Vec<(&'static str, &'static str)> = actions
            .iter()
            .map(|(k, n)| (*n, *k))
            .collect();
        col = col.child(dropdown_row(label, sig, options));
    }
    let body = col.child(muted(
        TextStyle::BODY_SMALL,
        "発火は隅に入ってから少し待ってから（150ms）です",
    ));
    page_shell(
        "ホットコーナー",
        section_card(icon(icons::CROP_SQUARE), "画面の隅にポインターを当てたとき", body),
    )
}

fn display_page(form: &Form, out_sel: Signal<usize>) -> Div {
    let names: Vec<String> = form.outputs.iter().map(|o| o.orig.name.clone()).collect();
    let selector = {
        let open = signal(false);
        let anchor = node_ref();
        let names_for_trigger = names.clone();
        let trigger = dyn_view(move || {
            let idx = out_sel.get().min(names_for_trigger.len().saturating_sub(1));
            let name = names_for_trigger.get(idx).cloned().unwrap_or_default();
            styled_button(name, ButtonStyle::tonal().icon(icons::ARROW_DROP_DOWN))
                .on_click(move |_| open.set(true))
                .node_ref(anchor)
        });
        let items: Vec<anti_webui_md3e::components::MenuItem> = names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                menu_item(name.clone())
                    .selected_when(move || out_sel.get() == i)
                    .on_click(move || {
                        out_sel.set(i);
                        open.set(false);
                    })
            })
            .collect();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(12.0)
            .child(static_txt(TextStyle::TITLE_SMALL, "出力"))
            .child(trigger)
            .child(styled_menu(
                move || open.get(),
                move || open.set(false),
                vec![items],
                MenuStyle::new().anchor(anchor).match_anchor_width(),
            ))
    };
    let forms = form.outputs.clone();
    let editor = dyn_view(move || {
        let idx = out_sel.get().min(forms.len().saturating_sub(1));
        let Some(o) = forms.get(idx) else {
            return div().child(muted(
                TextStyle::BODY_SMALL,
                "出力の設定が設定ファイルにありません",
            ));
        };
        div()
            .flex()
            .flex_col()
            .gap(12.0)
            .w_full()
            .child(slider_row_2dp("スケール", o.scale, 0.5..=2.5))
            .child(dyn_view({
                let scale_sig = o.scale;
                move || {
                    let s = scale_sig.get();
                    let px = (18.0 * s).clamp(8.0, 64.0);
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(12.0)
                        .child(div().w(px).h(px).dyn_style(|st| st.bg(color_scheme().primary)))
                        .child(muted(
                            TextStyle::BODY_SMALL,
                            format!("スケール {s:.2} の見本（四角の大きさ）"),
                        ))
                }
            }))
            .child(switch_row("可変リフレッシュレート (VRR)", o.vrr))
            .child(switch_row("HDR", o.hdr_on))
            .child(switch_row("ダイレクトスキャンアウト", o.scanout))
            .child(dropdown_row(
                "サブピクセル",
                o.subpixel,
                vec![
                    ("既定", ""),
                    ("不明", "unknown"),
                    ("なし", "none"),
                    ("横RGB", "horizontal-rgb"),
                    ("横BGR", "horizontal-bgr"),
                    ("縦RGB", "vertical-rgb"),
                    ("縦BGR", "vertical-bgr"),
                ],
            ))
            .child(
                text_field(o.icc, TextFieldStyle::new().label("ICC プロファイルのパス"))
                    .w_full(),
            )
            .child(muted(
                TextStyle::BODY_SMALL,
                "VRR は対応するディスプレイでのみ効きます。切り替え時に一瞬暗転することがあります",
            ))
    });
    let render = div()
        .flex()
        .flex_col()
        .gap(12.0)
        .w_full()
        .child(chips_row(
            "フレーム制御",
            form.pacing,
            vec![("スループット", "throughput"), ("低遅延", "low-latency")],
        ))
        .child(switch_row("起動時から FPS を表示", form.fps));
    page_shell(
        "画面",
        div()
            .flex()
            .flex_col()
            .gap(16.0)
            .w_full()
            .child(section_card(icon(icons::MONITOR), "出力", div().flex().flex_col().gap(16.0).w_full().child(selector).child(editor)))
            .child(section_card(icon(icons::ANIMATION), "描画", render)),
    )
}

fn startup_page(form: &Form, entries_ok: bool, new_name: Signal<String>, new_cmd: Signal<String>, new_service: Signal<bool>) -> Div {
    let entries_sig = form.entries;
    let list = dyn_view(move || {
        let items = entries_sig.with(|v| v.clone());
        let mut col = div().flex().flex_col().gap(8.0).w_full();
        if items.is_empty() {
            col = col.child(muted(
                TextStyle::BODY_SMALL,
                "まだ登録がありません。下の欄から追加できます",
            ));
        }
        for (idx, e) in items.into_iter().enumerate() {
            let kind = if e.service { "常駐" } else { "起動時" };
            col = col.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(12.0)
                    .w_full()
                    .child(
                        div()
                            .flex_1()
                            .min_w(0.0)
                            .flex()
                            .flex_col()
                            .gap(2.0)
                            .child(static_txt(TextStyle::TITLE_SMALL, e.name.clone()))
                            .child(muted(TextStyle::BODY_SMALL, format!("{kind}：{}", e.command))),
                    )
                    .child(styled_button("削除", ButtonStyle::text()).on_click(move |_| {
                        let mut v = entries_sig.with(|v| v.clone());
                        if idx < v.len() {
                            v.remove(idx);
                        }
                        entries_sig.set(v);
                    })),
            );
        }
        col
    });
    let add = div()
        .flex()
        .flex_col()
        .gap(12.0)
        .w_full()
        .child(text_field(new_name, TextFieldStyle::new().label("名前（空欄なら自動）")).w_full())
        .child(text_field(new_cmd, TextFieldStyle::new().label("コマンド")).w_full())
        .child(switch_row("常駐させる（終了したら再起動）", new_service))
        .child(
            div()
                .flex()
                .flex_row()
                .justify_end()
                .child(styled_button("追加", ButtonStyle::filled()).on_click(move |_| {
                    let cmd = new_cmd.with(|s| s.trim().to_string());
                    if cmd.is_empty() {
                        return;
                    }
                    let mut name = new_name.with(|s| s.trim().to_string());
                    if name.is_empty() {
                        let first = cmd.split_whitespace().next().unwrap_or("command");
                        let slug: String = first
                            .chars()
                            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                            .collect();
                        name = format!("user-{slug}");
                    }
                    {
                        let mut v = entries_sig.with(|v| v.clone());
                        let base = name.clone();
                        let mut candidate = base.clone();
                        let mut n = 2;
                        while v.iter().any(|e| e.name == candidate) {
                            candidate = format!("{base}-{n}");
                            n += 1;
                        }
                        v.push(AutoEntry {
                            name: candidate,
                            command: cmd.clone(),
                            service: new_service.get(),
                        });
                        entries_sig.set(v);
                    }
                    new_name.set(String::new());
                    new_cmd.set(String::new());
                    new_service.set(false);
                })),
        );
    let mut body = div().flex().flex_col().gap(16.0).w_full().child(list);
    if entries_ok {
        body = body.child(add);
    } else {
        body = body.child(muted(
            TextStyle::BODY_SMALL,
            "autostart.ts の形式が想定と違うため、起動コマンドは編集できません",
        ));
    }
    let body = body.child(muted(
        TextStyle::BODY_SMALL,
        "ShojiWM 本体やバーの自動起動（fcitx5 など）は対象外です。ここに登録したものだけを管理します",
    ));
    page_shell(
        "起動",
        section_card(icon(icons::ROCKET_LAUNCH), "ログイン時に実行するコマンド", body),
    )
}

fn power_page(form: &Form) -> Div {
    let lid_options: Vec<(&'static str, &'static str)> = vec![
        ("既定", ""),
        ("logind", "logind"),
        ("サスペンド", "suspend"),
        ("ハイバネート", "hibernate"),
        ("何もしない", "ignore"),
    ];
    let mut col = div().flex().flex_col().gap(12.0).w_full();
    if matches!(form.lid_orig.0, LidField::Function) {
        col = col.child(muted(
            TextStyle::BODY_SMALL,
            "バッテリー時：関数で指定されているため変更できません（今はサスペンド）",
        ));
    } else {
        col = col.child(dropdown_row("バッテリー時", form.lid_battery, lid_options.clone()));
    }
    if matches!(form.lid_orig.1, LidField::Function) {
        col = col.child(muted(
            TextStyle::BODY_SMALL,
            "外部電源時：関数で指定されているため変更できません",
        ));
    } else {
        col = col.child(dropdown_row("外部電源時", form.lid_external, lid_options));
    }
    let col = col
        .child(switch_row("外部画面があるときは内蔵画面を消す", form.lid_panel_off))
        .child(muted(
            TextStyle::BODY_SMALL,
            "ノートPC の蓋を閉じたときの動作です。「既定」は logind の設定に従います",
        ));
    page_shell(
        "電源",
        section_card(icon(icons::POWER_SETTINGS_NEW), "蓋を閉じたとき", col),
    )
}

fn backup_page(backups: Signal<Vec<PathBuf>>, pending: Signal<Option<PathBuf>>) -> Div {
    let list = dyn_view(move || {
        let items = backups.with(|v| v.clone());
        let mut col = div().flex().flex_col().gap(8.0).w_full();
        if items.is_empty() {
            col = col.child(muted(
                TextStyle::BODY_SMALL,
                "まだバックアップがありません（保存すると自動で作られます）",
            ));
        }
        for path in items.into_iter().take(20) {
            let label = config::backup_display_name(&path);
            let p = path.clone();
            col = col.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(12.0)
                    .w_full()
                    .child(
                        div()
                            .flex_1()
                            .min_w(0.0)
                            .child(static_txt(TextStyle::TITLE_SMALL, label)),
                    )
                    .child(
                        styled_button("復元", ButtonStyle::tonal())
                            .on_click(move |_| pending.set(Some(p.clone()))),
                    ),
            );
        }
        col
    });
    page_shell(
        "バックアップ",
        section_card(icon(icons::BACKUP), "保存時の自動バックアップ", list),
    )
}

fn about_page(wiring: (bool, bool)) -> Div {
    let commit = config::shojiwm_commit().unwrap_or_else(|| "不明".to_string());
    let body = div()
        .flex()
        .flex_col()
        .gap(8.0)
        .w_full()
        .child(static_txt(
            TextStyle::TITLE_SMALL,
            format!("shoji-settings {}", env!("CARGO_PKG_VERSION")),
        ))
        .child(muted(
            TextStyle::BODY_SMALL,
            format!("設定ファイル：{}", config::config_path().display()),
        ))
        .child(muted(
            TextStyle::BODY_SMALL,
            format!("起動コマンド：{}", config::autostart_path().display()),
        ))
        .child(muted(
            TextStyle::BODY_SMALL,
            format!(
                "組み込み：起動コマンド {}／ホットコーナー {}",
                if wiring.0 { "済み" } else { "未" },
                if wiring.1 { "済み" } else { "未" }
            ),
        ))
        .child(muted(
            TextStyle::BODY_SMALL,
            format!("ShojiWM の commit：{commit}"),
        ))
        .child(muted(
            TextStyle::BODY_SMALL,
            "設定の変更は、次回ログイン（ShojiWM の起動）から反映されます",
        ));
    page_shell(
        "について",
        section_card(icon(icons::INFO), "このアプリについて", body),
    )
}

pub fn root() -> Div {
    let text0 = config::read_config_text();
    let initial = text0.as_deref().and_then(config::parse);
    let readable = initial.is_some();
    let base0 = initial.clone().unwrap_or_default();
    let entries0 = config::read_autostart_text()
        .and_then(|t| config::parse_autostart(&t));
    let entries_ok = entries0.is_some() || !config::autostart_path().exists();
    let entries_init = entries0.clone().unwrap_or_default();

    let form = std::rc::Rc::new(std::cell::RefCell::new(Form::new(&base0, entries_init.clone())));
    let snapshot: Signal<Option<WmConfig>> = signal(initial);
    let snapshot_entries: Signal<Vec<AutoEntry>> = signal(entries_init);
    let page = signal(0usize);
    let out_sel = signal(0usize);
    let backups: Signal<Vec<PathBuf>> = signal(config::list_backups());
    let pending: Signal<Option<PathBuf>> = signal(None);
    let bars = snackbars();
    let themes = config::installed_cursor_themes();
    let new_name: Signal<String> = signal(String::new());
    let new_cmd: Signal<String> = signal(String::new());
    let new_service: Signal<bool> = signal(false);
    let wiring = text0
        .as_deref()
        .map(config::wiring_status)
        .unwrap_or((false, false));

    let form_for_dirty = form.clone();
    let dirty = memo(move || {
        let f = form_for_dirty.borrow();
        let cfg_dirty = match snapshot.with(|s| s.clone()) {
            Some(saved) => f.values(&saved) != saved,
            None => false,
        };
        let ent_dirty = entries_ok && f.entries() != snapshot_entries.with(|v| v.clone());
        cfg_dirty || ent_dirty
    });

    // 保存：ファイルを読み直してから反映し、検証を通ったものだけ書き込む。
    let form_for_save = form.clone();
    let save_bars = bars.clone();
    let on_save = move || {
        let f = form_for_save.borrow();
        // index.tsx 側
        if let Some(saved) = snapshot.with(|s| s.clone()) {
            let cfg = f.values(&saved);
            if cfg != saved {
                let Some(text) = config::read_config_text() else {
                    save_bars.show(Snackbar::new("設定ファイルが見つかりません"));
                    return;
                };
                match config::apply(&text, &cfg) {
                    Some(new_text) => match config::save(&new_text) {
                        Ok(_) => {
                            snapshot.set(Some(cfg));
                            backups.set(config::list_backups());
                        }
                        Err(_) => {
                            save_bars.show(Snackbar::new("保存に失敗しました"));
                            return;
                        }
                    },
                    None => {
                        save_bars.show(Snackbar::new(
                            "設定ファイルの形式が想定と違うため、保存できませんでした",
                        ));
                        return;
                    }
                }
            }
        }
        // autostart.ts 側
        if entries_ok {
            let entries = f.entries();
            if entries != snapshot_entries.with(|v| v.clone()) {
                match config::save_autostart(&entries) {
                    Ok(()) => {
                        snapshot_entries.set(entries);
                        backups.set(config::list_backups());
                    }
                    Err(_) => {
                        save_bars.show(Snackbar::new("起動コマンドの保存に失敗しました"));
                        return;
                    }
                }
            }
        }
        save_bars.show(Snackbar::new(
            "保存しました（次回ログインから反映されます）",
        ));
    };
    let on_save = std::rc::Rc::new(on_save);

    // 復元：今のファイルもバックアップしてから戻し、画面の値も読み直す。
    let form_for_restore = form.clone();
    let restore_bars = bars.clone();
    let on_restore = move || {
        let Some(path) = pending.with(|p| p.clone()) else {
            return;
        };
        pending.set(None);
        match config::restore(&path) {
            Ok(()) => {
                if let Some(cfg) = config::read_config_text()
                    .as_deref()
                    .and_then(config::parse)
                {
                    let entries = config::read_autostart_text()
                        .and_then(|t| config::parse_autostart(&t))
                        .unwrap_or_default();
                    form_for_restore.borrow_mut().load(&cfg, entries.clone());
                    snapshot.set(Some(cfg));
                    snapshot_entries.set(entries);
                }
                backups.set(config::list_backups());
                restore_bars.show(Snackbar::new(
                    "復元しました（次回ログインから反映されます）",
                ));
            }
            Err(_) => restore_bars.show(Snackbar::new("復元に失敗しました")),
        }
    };
    let on_restore = std::rc::Rc::new(on_restore);

    let warning = if readable {
        div()
    } else {
        filled_card()
            .flex()
            .flex_row()
            .gap(12.0)
            .p(16.0)
            .w_full()
            .child(static_txt(
                TextStyle::TITLE_SMALL,
                "設定ファイルを読み取れませんでした。表示は既定値で、保存はできません",
            ))
    };

    let form_c = form.borrow().clone();
    let pages = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(0.0)
        .w_full()
        .overflow_y_scroll()
        .child(warning)
        .child(show(move || page.get() == 0, {
            let f = form_c.clone();
            let themes = themes.clone();
            move || cursor_page(&f, themes.clone())
        }))
        .child(show(move || page.get() == 1, {
            let f = form_c.clone();
            move || keyboard_page(&f)
        }))
        .child(show(move || page.get() == 2, {
            let f = form_c.clone();
            move || pointer_page(&f)
        }))
        .child(show(move || page.get() == 3, {
            let f = form_c.clone();
            move || display_page(&f, out_sel)
        }))
        .child(show(move || page.get() == 4, {
            let f = form_c.clone();
            move || corners_page(&f)
        }))
        .child(show(move || page.get() == 5, {
            let f = form_c.clone();
            move || startup_page(&f, entries_ok, new_name, new_cmd, new_service)
        }))
        .child(show(move || page.get() == 6, {
            let f = form_c.clone();
            move || power_page(&f)
        }))
        .child(show(move || page.get() == 7, move || {
            backup_page(backups, pending)
        }))
        .child(show(move || page.get() == 8, move || about_page(wiring)));
    let content = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(0.0)
        .h_full()
        .child(pages)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_end()
                .gap(12.0)
                .p(16.0)
                .w_full()
                .child(muted(
                    TextStyle::BODY_SMALL,
                    "変更は次回ログインから反映されます",
                ))
                .child(
                    styled_button(
                        "保存",
                        ButtonStyle::filled()
                            .icon(icons::SAVE)
                            .disabled_when(move || !dirty.get()),
                    )
                    .on_click(move |_| on_save()),
                ),
        );

    let restore_dialog = dialog(
        move || pending.with(|p| p.is_some()),
        move || pending.set(None),
        move || {
            let label = pending
                .with(|p| p.clone())
                .map(|p| config::backup_display_name(&p))
                .unwrap_or_default();
            let on_restore = on_restore.clone();
            div()
                .flex()
                .flex_col()
                .gap(16.0)
                .p(24.0)
                .child(static_txt(TextStyle::TITLE_MEDIUM, "バックアップに戻しますか？"))
                .child(muted(TextStyle::BODY_SMALL, label))
                .child(muted(
                    TextStyle::BODY_SMALL,
                    "今の設定もバックアップしてから戻します",
                ))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap(8.0)
                        .child(
                            styled_button("キャンセル", ButtonStyle::text())
                                .on_click(move |_| pending.set(None)),
                        )
                        .child(
                            styled_button("復元する", ButtonStyle::filled())
                                .on_click(move |_| on_restore()),
                        ),
                )
        },
    );

    div()
        .flex()
        .flex_row()
        .size_full()
        .dyn_style(|s| {
            s.bg(color_scheme().surface)
                .text_color(color_scheme().on_surface)
        })
        .child(navigation_rail(
            vec![
                nav_item(icons::ARROW_SELECTOR_TOOL, "カーソル"),
                nav_item(icons::KEYBOARD, "キーボード"),
                nav_item(icons::MOUSE, "ポインター"),
                nav_item(icons::MONITOR, "画面"),
                nav_item(icons::CROP_SQUARE, "コーナー"),
                nav_item(icons::ROCKET_LAUNCH, "起動"),
                nav_item(icons::POWER_SETTINGS_NEW, "電源"),
                nav_item(icons::BACKUP, "バックアップ"),
                nav_item(icons::INFO, "について"),
            ],
            move || page.get(),
            move |i| page.set(i),
        ))
        .child(content)
        .child(restore_dialog)
        .child(bars.view())
}
