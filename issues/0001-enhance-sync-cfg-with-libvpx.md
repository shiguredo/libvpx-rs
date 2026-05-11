# Encoder::cfg と libvpx 内部 cfg の乖離を解消する

Created: 2026-05-11

Model: Opus 4.7

## 背景

`Encoder::cfg` (`src/lib.rs`) はラッパー側でユーザー入力ベースの `vpx_codec_enc_cfg`
を保持する。一方 libvpx は `vp9_cx_iface.c` の `set_encoder_config()` などで入力
cfg を破壊的にクリップする場合がある。具体的には次のような silent clip がある。

- VP9: `cfg->rc_target_bitrate = VPXMIN(VPXMIN(raw_target_rate, cfg->rc_target_bitrate), 1000000);`
- VP8: `oxcf->target_bandwidth = VPXMIN(cfg.rc_target_bitrate, 1000000);`

`raw_target_rate` は `w * h * bit_depth * 3 * framerate / 1000` などフレーム特性に
依存する。

## 課題

ラッパー側の `self.cfg.rc_target_bitrate` と libvpx 内部の
`priv->cfg.rc_target_bitrate` が乖離する。次の `reconfigure` で
`target_bitrate: None` を指定した場合、`self.cfg` をベースにした `new_cfg` が
クリップ前の値で再送され、libvpx が再度同じクリップを行う。`raw_target_rate` が
初期化時と reconfigure 時で変わるシナリオでは、ユーザーが意図しない `target_bitrate`
が再度上書きされる可能性がある。

## 根拠

`/review-diff-code` のレビュー (Imp-1) で発見された乖離。対応 PR
(`feature/change-reconfigure`) では `Encoder::cfg` のフィールド doc に「ユーザー
入力ベースの値であり libvpx 内部とは一致しない」と注記したが、根本対応は別 issue
に持ち越した。

## 対応案

以下のいずれか。

1. `reconfigure` 成功後、libvpx の getter (`vpx_codec_get_*` 系) で内部状態を
   読み戻して `self.cfg` を上書きする。ただし getter API は項目が限定されており、
   `rc_target_bitrate` 等の取得手段が存在するか要調査。
2. `Encoder::cfg` を「常にユーザー入力スナップショット」として明示し、reconfigure
   時には `self.cfg` ではなくフィールド単位のラッパー保持値を更新する設計に変更
   する。`vpx_codec_enc_cfg` 全体を保持する現在の素朴な設計を再検討する。
3. ドキュメントだけで運用する (現状)。利用者に「reconfigure を連続呼び出しすると
   libvpx 内部の値が暗黙の影響を受け得る」ことを伝える。

優先度は中。実害が出るのは「reconfigure を連続して呼び出し、解像度やフレームレート
を間に変える」ような複合シナリオに限られる。
