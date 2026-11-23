mod config;
mod ui;

use anti_webui::prelude::*;
use anti_webui_md3e::{ColorScheme, SchemeOptions, SchemeVariant, set_color_scheme};

fn variant_of(name: &str) -> SchemeVariant {
    match name {
        "vibrant" => SchemeVariant::Vibrant,
        "expressive" => SchemeVariant::Expressive,
        "fidelity" => SchemeVariant::Fidelity,
        "neutral" => SchemeVariant::Neutral,
        "monochrome" => SchemeVariant::Monochrome,
        _ => SchemeVariant::TonalSpot,
    }
}

fn main() {
    // 表示なしで設定の読み書きだけを確かめる確認モード（開発・検証用）。
    if std::env::args().nth(1).as_deref() == Some("--check-config") {
        let Some(text) = config::read_config_text() else {
            println!("CONFIG_FILE_NOT_FOUND");
            return;
        };
        let Some(cfg) = config::parse(&text) else {
            println!("PARSE_FAILED");
            return;
        };
        println!("PARSED: {cfg:?}");
        match config::apply(&text, &cfg) {
            Some(new_text) => {
                println!(
                    "ROUNDTRIP: {}",
                    if new_text == text { "IDENTICAL" } else { "DIFFERS" }
                );
            }
            None => println!("ROUNDTRIP: APPLY_FAILED"),
        }
        println!("THEMES: {:?}", config::installed_cursor_themes());
        println!("BACKUPS: {}", config::list_backups().len());
        let (w_auto, w_hot) = config::wiring_status(&text);
        println!("WIRING: autostart={w_auto} hotcorners={w_hot}");
        match config::read_autostart_text() {
            Some(auto_text) => match config::parse_autostart(&auto_text) {
                Some(entries) => {
                    println!("AUTOSTART_ENTRIES: {}", entries.len());
                    println!(
                        "AUTOSTART_ROUNDTRIP: {}",
                        if config::generate_autostart(&entries) == auto_text {
                            "IDENTICAL"
                        } else {
                            "DIFFERS"
                        }
                    );
                }
                None => println!("AUTOSTART: PARSE_FAILED"),
            },
            None => println!("AUTOSTART: FILE_NOT_FOUND"),
        }
        return;
    }

    // 書き込み経路の自己テスト（メモリ内だけ。ファイルは /tmp へ参考出力）。
    if std::env::args().nth(1).as_deref() == Some("--self-test") {
        let Some(text) = config::read_config_text() else {
            println!("SELFTEST: NO_FILE");
            return;
        };
        let Some(mut cfg) = config::parse(&text) else {
            println!("SELFTEST: PARSE_FAILED");
            return;
        };
        cfg.touchpad.click_method = Some("clickfinger".to_string());
        cfg.touchpad.scroll_factor = 0.5;
        cfg.pointer.left_handed = Some(true);
        cfg.pointer.accel = 0.2;
        cfg.outputs[0].vrr = Some(true);
        cfg.outputs[0].hdr = config::Hdr::Off;
        cfg.outputs[1].hdr = config::Hdr::On;
        cfg.outputs[2].scale = 1.75;
        cfg.outputs[2].subpixel = Some("horizontal-rgb".to_string());
        cfg.outputs[2].icc = Some("/tmp/test.icc".to_string());
        cfg.frame_pacing = "low-latency".to_string();
        cfg.fps_initial = true;
        cfg.lid.on_external = config::LidField::Value("suspend".to_string());
        cfg.lid.internal_panel_off = false;
        if let Some(mut c) = cfg.hotcorners {
            c[0] = config::CornerAction::Lock;
            c[2] = config::CornerAction::WorkspaceNext;
            cfg.hotcorners = Some(c);
        }
        cfg.kb_options = String::new();
        match config::apply(&text, &cfg) {
            Some(out) => {
                let re = config::parse(&out);
                println!(
                    "SELFTEST_APPLY: {}",
                    if re.as_ref() == Some(&cfg) { "OK" } else { "MISMATCH" }
                );
                if re.as_ref() != Some(&cfg) {
                    println!("EXPECTED: {cfg:?}");
                    println!("GOT: {re:?}");
                }
                let _ = std::fs::write("/tmp/selftest-before.tsx", &text);
                let _ = std::fs::write("/tmp/selftest-after.tsx", &out);
            }
            None => println!("SELFTEST_APPLY: FAILED"),
        }
        let entries = vec![
            config::AutoEntry {
                name: "user-notify".to_string(),
                command: "notify-send \"a b\" test".to_string(),
                service: false,
            },
            config::AutoEntry {
                name: "user-svc".to_string(),
                command: "mydaemon --flag".to_string(),
                service: true,
            },
        ];
        let gen = config::generate_autostart(&entries);
        println!(
            "SELFTEST_AUTOSTART: {}",
            if config::parse_autostart(&gen) == Some(entries) {
                "OK"
            } else {
                "MISMATCH"
            }
        );
        return;
    }

    // autostart.ts を（再）生成する。既存の内容は読み直して引き継ぐ。
    if std::env::args().nth(1).as_deref() == Some("--write-autostart") {
        let entries = config::read_autostart_text()
            .and_then(|t| config::parse_autostart(&t))
            .unwrap_or_default();
        match config::save_autostart(&entries) {
            Ok(()) => println!("AUTOSTART_WRITTEN: {} entries", entries.len()),
            Err(e) => println!("AUTOSTART_WRITE_FAILED: {e}"),
        }
        return;
    }

    // バー（shoji-bar-4）が今使っている配色に合わせる。読めなければ Material 基準の既定。
    let theme = config::load_bar_theme();
    let seed = theme.seed;
    let variant = variant_of(&theme.palette);
    let dark = theme.dark;
    let _ = App::new()
        .title("ShojiWM 設定")
        .app_id("shoji-settings")
        .size(1040, 720)
        .run(move || {
            set_color_scheme(ColorScheme::from_seed_with(
                rgb(seed),
                SchemeOptions {
                    variant,
                    dark,
                    ..Default::default()
                },
            ));
            ui::root()
        });
}
