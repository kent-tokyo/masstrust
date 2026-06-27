# masstrust

MS/MS 分子注释的置信度校准与弃权工具包。

[![CI](https://github.com/kent-tokyo/masstrust/actions/workflows/ci.yml/badge.svg)](https://github.com/kent-tokyo/masstrust/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

[English](README.md) | [日本語](README_ja.md)

---

## masstrust 是什么

`masstrust` 是一个用于 MS/MS 分子注释中**选择性预测 (selective prediction)** 的 Rust 工具包。

它接收外部注释或检索工具生成的候选排名，并在用户指定的错误率目标下，决定是否**信任 (trust)** 或**弃权 (abstain)** 排名最高的分子注释。

核心理念：

> 信任高置信度的注释，在不确定性过高时选择弃权。

## masstrust 不做什么

- 不执行 MS/MS 数据库搜索或光谱匹配
- 不生成分子结构
- 不执行逆合成 (retrosynthesis)
- 不替代 SIRIUS、CSI:FingerID、MassSpecGym 等工具
- 不保证临床准确性

`masstrust` 是注释流程的**后验信任层 (post-hoc trust layer)**，位于注释工具之后，控制接受错误注释的风险。

---

## 安装

### Rust CLI（从源码构建）

```bash
git clone https://github.com/kent-tokyo/masstrust
cd masstrust
cargo install --path crates/masstrust-cli
```

### Python wheel（需要 [maturin](https://maturin.rs)）

```bash
maturin build --features extension-module
pip install target/wheels/masstrust-*.whl
```

---

## 快速开始 — CLI

**第 1 步.** 从带标签的候选数据计算风险-覆盖率曲线：

```bash
masstrust curve examples/labeled_candidates.csv \
  --score score-gap \
  --out risk_coverage.csv \
  --verbose
```

**第 2 步.** 在 5% 错误率目标下校准阈值：

```bash
masstrust calibrate examples/labeled_candidates.csv \
  --score score-gap \
  --error-rate 0.05 \
  --method empirical \
  --out policy.json
```

**第 3 步.** 将策略应用于新的（无标签）候选数据：

```bash
masstrust apply examples/candidates.csv \
  --policy policy.json \
  --out trusted_annotations.csv \
  --abstained abstained.csv
```

**批处理模式** — 对多个文件批量应用策略：

```bash
masstrust batch data/*.csv \
  --policy policy.json \
  --out-dir ./results/
```

### 示例输出

```
$ masstrust calibrate examples/massspecgym_candidates.csv \
    --score score-gap --error-rate 0.05 --method empirical --out policy.json

校准结果 (ScoreGap, empirical):
  目标错误率：  0.0500
  阈值：        0.120000
  覆盖率：      0.5000（8 条查询中接受 4 条，50.0%）
  观测风险：    0.0000（0 个错误接受 / 4 个接受）
  AURC：        0.151488
  E-AURC：      -0.001938

$ masstrust apply examples/candidates.csv --policy policy.json \
    --out trusted.csv --abstained abstained.csv

接受：1  弃权：1
```

**SVG 图表**（需要 `--features plot`）：

```bash
cargo build --features plot
masstrust curve examples/labeled_candidates.csv \
  --score score-gap --out risk.csv \
  --plot risk.svg --histogram hist.svg
```

---

## 快速开始 — Python

```python
import masstrust

# 1. 计算风险-覆盖率曲线
curve = masstrust.compute_curve(
    "examples/labeled_candidates.csv",
    score="score-gap",
)
print(f"AURC: {masstrust.aurc(curve):.4f}")

# 2. 校准
policy = masstrust.calibrate(
    "examples/labeled_candidates.csv",
    score="score-gap",
    error_rate=0.05,
    method="empirical",
)
print(f"阈值: {policy['threshold']:.4f}")

# 3. 应用于新数据
decisions = masstrust.apply_policy("examples/candidates.csv", policy)
accepted  = [d for d in decisions if d["accepted"]]
print(f"接受: {len(accepted)}/{len(decisions)}")

# 保存 / 加载策略
masstrust.save_policy("policy.json", policy)
policy = masstrust.load_policy("policy.json")
```

---

## 输入 CSV 格式

```csv
query_id,candidate_id,rank,score,probability,is_correct
q1,cand_a,1,0.92,0.71,true
q1,cand_b,2,0.81,0.21,false
q2,cand_c,1,0.88,0.46,false
q2,cand_d,2,0.86,0.43,true
```

| 列名 | 必填 | 说明 |
|------|------|------|
| `query_id` | ✓ | 光谱标识符 |
| `candidate_id` | ✓ | 候选结构标识符 |
| `rank` | ✓ | 该查询中的排名（1 = 最佳） |
| `score` | ✓ | 原始注释得分（越高越好） |
| `probability` | — | 后验概率（`max-prob`、`margin`、`entropy` 所需） |
| `is_correct` | — | 真实标签（`calibrate` 和 `curve` 所需） |

通过 `--features parquet` 支持 Parquet 格式输入（按 `.parquet` 扩展名自动检测）。

---

## 置信度评分方法

| 方法 | 计算公式 | 所需条件 |
|------|---------|---------|
| `score-gap` | `score(第1位) − score(第2位)` | ≥2 个候选 |
| `max-prob` | `probability(第1位)` | `probability` 列 |
| `margin` | `probability(第1位) − probability(第2位)` | `probability` 列，≥2 个候选 |
| `entropy` | `1 − H_normalized`（所有候选概率的归一化熵） | 所有候选的 `probability` 列 |

---

## 校准方法

| 方法 | 行为 | 备注 |
|------|------|------|
| `empirical` | 选择观测风险 ≤ 目标值时覆盖率最大的阈值 | 无验证集之外的统计保证 |
| `binomial` | 选择 Wilson 上界 ≤ 目标值时覆盖率最大的阈值 | 保守型；添加 `--confidence-level 0.95` |
| `crc` *（实验性）* | 将经验目标收紧 `1/(n+1)` 后选择阈值 | CRC 风格校正；见下方注意事项 |

```bash
# 经验型
masstrust calibrate labeled.csv --score score-gap --error-rate 0.05 --method empirical --out policy.json

# 二项式（保守型）
masstrust calibrate labeled.csv --score score-gap --error-rate 0.05 --method binomial --confidence-level 0.95 --out policy.json

# CRC 风格（实验性）
masstrust calibrate labeled.csv --score score-gap --error-rate 0.05 --method crc --out policy.json
```

---

## 理解风险与覆盖率

**风险-覆盖率曲线 (risk-coverage curve)** 描述了接受/弃权之间的权衡：

- **覆盖率 (Coverage)** = 预测被接受的查询比例
- **风险 (Risk)** = 被接受的预测中错误的比例

在任意阈值下，更高的覆盖率意味着更高的风险。`masstrust` 在将风险控制在指定目标内的同时，寻找最大化覆盖率的阈值。

```
风险
1.0 ┤                          ╭──
    │                     ╭────╯
    │                ╭────╯
0.05┤ ─ ─ ─ ─ ─ ─ ─╱── 目标
    │           ╭───╯
0.0 ┤───────────╯
    └──────────────────────────── 覆盖率
    0                             1.0
```

阈值选择为曲线上处于目标风险线之下的最右端点。

---

## 策略 JSON

校准后的策略保存为可复现的 JSON 文件：

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

## 科学注意事项

- `masstrust` **在所提供验证数据上，基于所选校准程序控制观测风险或有界风险**。对分布外光谱的正确性不提供保证。
- 风险控制基于所提供的验证集进行校准。若测试分布不同（不同仪器、加合物类型、化合物类别），保证可能不适用。
- 实验性的 `crc` 方法假设校准数据独立同分布 (i.i.d.) 且损失函数为二值（0/1：正确/错误）。在依赖该保证之前，请验证您的数据集满足这些假设。
- 小规模校准集（约 20 条以下）可能产生非常保守的阈值，或找不到有效阈值（报告为 `threshold = +inf`，即全部弃权）。
- `masstrust` 不声称临床有效性或符合监管要求。

---

## 参考文献

- Angelopoulos, A. N., Bates, S., Fisch, A., Lei, L., & Schuster, R. (2022). **Conformal Risk Control.** *arXiv:2208.02814.* — `crc` 校准方法的理论基础。
- Geifman, Y., & El-Yaniv, R. (2017). **Selective classification for deep neural networks.** *NeurIPS.* — 选择性预测的基础性框架。

---

## 许可证

在以下任一许可证下提供：

- [MIT 许可证](LICENSE-MIT)
- [Apache 许可证 2.0 版本](LICENSE-APACHE)

您可以任选其一。
