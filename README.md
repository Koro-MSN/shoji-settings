# shoji-settings

ShojiWM の詳細設定アプリ（v0.3.2）。[AntiWebUI](https://github.com/bea4dev/AntiWebUI) ＋ Material 3 Expressive で作った Rust 製 GUI です。

## 概要

ShojiWM の設定ファイル（`~/.config/shojiwm/src/index.tsx`）と、バーの設定（`$XDG_STATE_HOME/shoji-bar-4/settings.json`）を読み書きします。設定ファイルの形式を保ったまま書き換える方式で、バックアップの作成・復元にも対応しています。

## できること

- 出力（モニター）設定
- コーナーアクション、リッド（蓋）動作などの WM 動作設定
- 自動起動エントリの編集
- カラースキーム（Material 3 のバリアント：TonalSpot / Vibrant / Expressive / Fidelity / Neutral / Monochrome）とシード色
- カーソルテーマの選択（インストール済みテーマから）
- 設定バックアップの一覧と復元

## ビルド

```sh
cargo build --release
```

依存の AntiWebUI は GitHub（bea4dev/AntiWebUI）から git 依存で取得します。

## 確認モード

表示なしで設定の読み書きだけを検証できます（開発・検証用）。

```sh
shoji-settings --check-config
```

設定ファイルの読み込み・パース・書き戻しのラウンドトリップが一致するかを表示します。

## 構成

- `src/main.rs` — エントリポイント、カラースキーム適用、確認モード
- `src/config.rs` — 設定ファイル（index.tsx / settings.json）の読み書き
- `src/ui.rs` — 設定画面の UI とフォーム状態
