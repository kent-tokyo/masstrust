# masstrust

MS/MS 分子アノテーションのための信頼度調整と棄権判定ツールキット。

[![CI](https://github.com/kent-tokyo/masstrust/actions/workflows/ci.yml/badge.svg)](https://github.com/kent-tokyo/masstrust/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

[English](README.md) | [中文](README_zh.md)

---

## masstrust とは

`masstrust` は MS/MS 分子アノテーションにおける**選択的予測 (selective prediction)** のための Rust ツールキットです。

外部のアノテーションツールや検索ツールが出力した候補ランキングを受け取り、ユーザーが指定したエラーレート目標のもとで、上位候補のアノテーションを**信頼 (trust)** するか**棄権 (abstain)** するかを決定します。

中心的な考え方：

> 確信度の高いアノテーションは信頼し、不確実性が高い場合は棄権する。

## masstrust がやらないこと

- MS/MS データベース検索やスペクトルマッチングは行いません
- 分子構造の生成は行いません
- 逆合成 (retrosynthesis) は行いません
- SIRIUS、CSI:FingerID、MassSpecGym などのツールを置き換えるものではありません
- 臨床的な正確性を保証するものではありません

`masstrust` はアノテーションパイプラインのための**事後信頼層 (post-hoc trust layer)** です。任意のアノテーションツールの出力を受け取り、誤ったアノテーションを受理するリスクをコントロールします。

---

## インストール

### Rust CLI（ソースからビルド）

```bash
git clone https://github.com/kent-tokyo/masstrust
cd masstrust
cargo install --path crates/masstrust-cli
```

### Python wheel（[maturin](https://maturin.rs) が必要）

```bash
maturin build --features extension-module
pip install target/wheels/masstrust-*.whl
```

---

## クイックスタート — CLI

**ステップ 1.** ラベル付き候補からリスクカバレッジ曲線を計算：

```bash
masstrust curve examples/labeled_candidates.csv \
  --score score-gap \
  --out risk_coverage.csv \
  --verbose
```

**ステップ 2.** エラーレート 5% で閾値をキャリブレーション：

```bash
masstrust calibrate examples/labeled_candidates.csv \
  --score score-gap \
  --error-rate 0.05 \
  --method empirical \
  --out policy.json
```

**ステップ 3.** 新しい（ラベルなし）候補にポリシーを適用：

```bash
masstrust apply examples/candidates.csv \
  --policy policy.json \
  --out trusted_annotations.csv \
  --abstained abstained.csv
```

**バッチモード** — 複数ファイルに一括適用：

```bash
masstrust batch data/*.csv \
  --policy policy.json \
  --out-dir ./results/
```

**SVG プロット**（`--features plot` が必要）：

```bash
cargo build --features plot
masstrust curve examples/labeled_candidates.csv \
  --score score-gap --out risk.csv \
  --plot risk.svg --histogram hist.svg
```

---

## クイックスタート — Python

```python
import masstrust

# 1. リスクカバレッジ曲線を計算
curve = masstrust.compute_curve(
    "examples/labeled_candidates.csv",
    score="score-gap",
)
print(f"AURC: {masstrust.aurc(curve):.4f}")

# 2. キャリブレーション
policy = masstrust.calibrate(
    "examples/labeled_candidates.csv",
    score="score-gap",
    error_rate=0.05,
    method="empirical",
)
print(f"閾値: {policy['threshold']:.4f}")

# 3. 新しいデータに適用
decisions = masstrust.apply_policy("examples/candidates.csv", policy)
accepted  = [d for d in decisions if d["accepted"]]
print(f"受理: {len(accepted)}/{len(decisions)}")

# ポリシーの保存・読み込み
masstrust.save_policy("policy.json", policy)
policy = masstrust.load_policy("policy.json")
```

---

## 入力 CSV フォーマット

```csv
query_id,candidate_id,rank,score,probability,is_correct
q1,cand_a,1,0.92,0.71,true
q1,cand_b,2,0.81,0.21,false
q2,cand_c,1,0.88,0.46,false
q2,cand_d,2,0.86,0.43,true
```

| 列名 | 必須 | 説明 |
|------|------|------|
| `query_id` | ✓ | スペクトル識別子 |
| `candidate_id` | ✓ | 候補構造の識別子 |
| `rank` | ✓ | クエリ内でのランク（1 = 最上位） |
| `score` | ✓ | アノテーションスコア（高いほど良い） |
| `probability` | — | 事後確率（`max-prob`、`margin`、`entropy` に必要） |
| `is_correct` | — | 正解ラベル（`calibrate` と `curve` に必要） |

`--features parquet` オプションで Parquet 形式の入力も対応（`.parquet` 拡張子で自動検出）。

---

## 信頼スコアリング方法

| 方法 | 計算式 | 必要条件 |
|------|--------|---------|
| `score-gap` | `score(1位) − score(2位)` | 候補が2件以上 |
| `max-prob` | `probability(1位)` | `probability` 列 |
| `margin` | `probability(1位) − probability(2位)` | `probability` 列、候補が2件以上 |
| `entropy` | `1 − H_normalized`（全候補の確率に対するエントロピー） | 全候補の `probability` 列 |

---

## キャリブレーション方法

| 方法 | 動作 | 備考 |
|------|------|------|
| `empirical` | 観測リスク ≤ 目標となる最大カバレッジの閾値を選択 | 検証セット外の統計的保証なし |
| `binomial` | Wilson 上限 ≤ 目標となる最大カバレッジの閾値を選択 | 保守的；`--confidence-level 0.95` を追加 |
| `crc` *（実験的）* | 経験的目標を `1/(n+1)` だけ厳しくして閾値を選択 | CRC スタイルの補正；下記の注意事項を参照 |

```bash
# 経験的
masstrust calibrate labeled.csv --score score-gap --error-rate 0.05 --method empirical --out policy.json

# 二項（保守的）
masstrust calibrate labeled.csv --score score-gap --error-rate 0.05 --method binomial --confidence-level 0.95 --out policy.json

# CRC スタイル（実験的）
masstrust calibrate labeled.csv --score score-gap --error-rate 0.05 --method crc --out policy.json
```

---

## リスクとカバレッジの読み方

**リスクカバレッジ曲線 (risk-coverage curve)** は受理/棄権のトレードオフを表します：

- **カバレッジ (Coverage)** = 予測が受理されたクエリの割合
- **リスク (Risk)** = 受理された予測のうち誤っていたものの割合

閾値が低いほどカバレッジは上がりますが、リスクも上がります。`masstrust` はリスクを指定目標以内に保ちながら、カバレッジを最大化する閾値を探します。

```
リスク
1.0 ┤                          ╭──
    │                     ╭────╯
    │                ╭────╯
0.05┤ ─ ─ ─ ─ ─ ─ ─╱── 目標
    │           ╭───╯
0.0 ┤───────────╯
    └──────────────────────────── カバレッジ
    0                             1.0
```

目標ラインを下回る最も右の点が閾値として選択されます。

---

## ポリシー JSON

キャリブレーション済みポリシーは再現可能な JSON ファイルとして保存されます：

```json
{
  "version": "0.1.0",
  "scoring_method": "score_gap",
  "threshold": 0.18,
  "target_error_rate": 0.05,
  "calibration_method": "empirical",
  "confidence_level": null,
  "created_by": "masstrust"
}
```

---

## 科学的注意事項

- `masstrust` は**提供された検証データに対して、選択されたキャリブレーション手順のもとで観測リスクまたは上限リスクをコントロール**します。分布外スペクトルへの正確性は保証しません。
- リスクコントロールは提供された検証セットでキャリブレーションされます。テスト分布が異なる場合（機器の種類、付加体の種類、化合物クラスなど）、保証が転用できない場合があります。
- 実験的な `crc` 方法は、キャリブレーションデータが i.i.d. であり、損失関数が二値（0/1: 正解/不正解）であることを前提とします。データセットでこれらの仮定が成立するか確認してから利用してください。
- 小規模なキャリブレーションセット（約 20 件未満）では、非常に保守的な閾値になるか、有効な閾値が見つからない場合があります（`threshold = +inf`、つまりすべて棄権、と報告されます）。
- `masstrust` は臨床的有効性や規制適合性を主張するものではありません。

---

## 参考文献

- Angelopoulos, A. N., Bates, S., Fisch, A., Lei, L., & Schuster, R. (2022). **Conformal Risk Control.** *arXiv:2208.02814.* — `crc` キャリブレーション方法の基礎。
- Geifman, Y., & El-Yaniv, R. (2017). **Selective classification for deep neural networks.** *NeurIPS.* — 選択的予測の基礎的フレームワーク。

---

## ライセンス

以下のいずれかのライセンスで提供されます：

- [MIT ライセンス](LICENSE-MIT)
- [Apache ライセンス バージョン 2.0](LICENSE-APACHE)

いずれかをお選びください。
