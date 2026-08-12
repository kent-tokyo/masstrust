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

**比较多种评分方法**（在同一个带标签数据集上）：

```bash
masstrust compare examples/labeled_candidates.csv \
  --scores score-gap,score-ratio,topk-gap,candidate-count \
  --error-rate 0.05 --method empirical \
  --bootstrap 1000 \
  --out compare.csv
```

**在独立的留出数据上评估策略** — 在一个数据集（如验证集）上校准，在另一个数据集（如测试集）上评估，且不重新校准：

```bash
masstrust calibrate val.csv --score score-gap --error-rate 0.05 --method empirical --out policy.json
masstrust evaluate test.csv --policy policy.json --out eval_report.json
```

**检测校准数据与新数据之间的置信度漂移**：

```bash
masstrust drift --calibration examples/labeled_candidates.csv --new examples/candidates.csv \
  --score score-gap --out drift_report.json
```

**防止校准集/测试集之间的数据泄漏**（`query_id`/`inchikey`/`formula` 重叠检测）：

```bash
masstrust validate-split --calibration val.csv --test test.csv
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

在 `curve` 或 `compare` 上添加 `--bootstrap 1000` 可获得 AURC 的 95% 自助法（bootstrap）置信区间。

---

## 置信度评分方法

| 方法 | 计算公式 | 所需条件 |
|------|---------|---------|
| `score-gap` | `score(第1位) − score(第2位)` | ≥2 个候选 |
| `score-ratio` | `score(第1位) / score(第2位)` | ≥2 个候选，`score(第2位) > 0` |
| `topk-gap` | `score(第1位) − mean(score(第2..min(5,n)位))` | ≥2 个候选 |
| `candidate-count` | `1 / 候选数` | 无——始终可计算 |
| `max-prob` | `probability(第1位)` | `probability` 列 |
| `margin` | `probability(第1位) − probability(第2位)` | `probability` 列，≥2 个候选 |
| `entropy` | `1 − H_normalized`（所有候选概率的归一化熵） | 所有候选的 `probability` 列 |
| `effective-k` | `exp(−H)`（所有候选概率的熵） | 所有候选的 `probability` 列 |

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

## 批量选择性部署认证（`risksieve`，可选）

`masstrust certify-batch` 是一个 **feature-gated**（`--features risksieve`）的独立工作流，
构建于 [`risksieve`](https://crates.io/crates/risksieve) crate 的 SCoRE-SDR 控制器之上
（Bai and Jin, 2026, arXiv:2603.24704）。默认构建中完全不会暴露该命令。

**这不是可复用的阈值策略。** 与 `calibrate`/`apply` 不同，每次运行它都会针对特定的
校准集 + 测试批次这一对数据重新计算一次选择结果 —— 默认 construction 的 e-value 依赖于
整个测试批次的组成，因此同一份校准数据在不同的测试批次上可能会选出不同的子集。关于为什么
这是一个独立命令而不是新的 `CalibrationMethod`，以及 `GuaranteeKind::SelectiveDeploymentRisk`
具体主张与不主张什么，请参见
[`docs/risksieve-integration.md`](docs/risksieve-integration.md)。

```bash
masstrust certify-batch \
  --calibration val.csv \
  --test test.csv \
  --score score-gap \
  --alpha 0.05 \
  --gamma 0.05 \
  --construction coupled \
  --accepted accepted.csv \
  --abstained abstained.csv \
  --certificate certificate.json \
  --report report.md
```

- 风险保证依赖于明确声明的前提条件（校准集与*整个*测试批次的可交换性、损失落在 `[0, 1]`
  区间内等）—— `certificate.json` 和 `report.md` 都会完整记录这些前提条件；在信任任何数字之前
  请先阅读它们。
- **选中数量为零是正常且有效的认证结果**，不是错误 —— SCoRE-SDR 的界在空选择集合上也平凡成立。
- 如果 `test.csv` 恰好带有 `is_correct` 标签，报告中还会打印**实现选择性风险
  (realized selective risk)** —— 这是根据这一批次的实际结果计算出的事后描述性统计量，与上方
  由定理保证的认证结果明确区分开，且从不被描述为对该认证结果的验证（或反驳）。

### 分级化学损失（`--loss-column`）

默认情况下，被认证/实现的损失是二元的 top-1 正确率（`is_correct`：正确为 `0.0`，否则为
`1.0`）。指定 `--loss-column <name>` 后，可以改为针对任意落在 `[0, 1]` 区间内的预计算损失
进行认证 —— 例如 Tanimoto 不相似度或骨架不匹配，这些都在上游计算完成（masstrust 本身从不
计算化学性质；详见
[`docs/graded-loss-integration.md`](docs/graded-loss-integration.md)）：

```bash
masstrust certify-batch \
  --calibration val.csv \
  --test test.csv \
  --score score-gap \
  --alpha 0.05 \
  --gamma 0.05 \
  --loss-column tanimoto_loss \
  --accepted accepted.csv \
  --abstained abstained.csv \
  --certificate certificate.json \
  --report report.md
```

- 对每一条可评分的**校准**查询都是必需的（若该列不存在，或某个值缺失/格式错误/超出
  `[0, 1]`/非有限，都会硬性报错）。对 `--test` 则是真正可选的 —— 完全没有该列的无标签测试集
  也能成功完成认证，只是无法得出实现风险的数值。
- `certificate.json`/`report.md` 会记录 `loss_kind`/`loss_label`/`loss_column`/`loss_domain`，
  使得使用 `--loss-column` 得到的认证结果不会被误读为针对完全匹配风险的认证。
- 实现风险始终只针对认证时所用的*同一种*损失来解析 —— 若要求针对不同的 `--loss-column`
  （或针对分级损失认证结果要求二元正确率）来解析，会得到硬性报错，而不是悄悄返回一个不匹配
  的数字。

`masstrust` 现有的校准方法（`empirical`/`binomial`/`crc`）不受此功能影响 —— 下方的科学注意
事项同样适用于 `certify-batch`。

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

## 在 MassSpecGym 上进行基准测试

`benchmarks/massspecgym/` 是一个独立于 Rust 工作区之外的自包含流水线（依赖 `massspecgym`/torch/rdkit），用于训练官方的 Fingerprint FFN 检索基线模型，将其预测结果导出为 masstrust 的数据格式，并报告 masstrust 自身评分方法在真实 MassSpecGym 数据上的 AURC / E-AURC / 目标风险下的覆盖率。这是在与竞品比较之前，建立一个真实、可复现的基准——目前尚未声称与任何竞品进行了比较。

该流水线本身已通过一次小规模的真实数据 preflight 运行（下载 → 训练 → 保存检查点 → 导出预测 → 生成报告）完成端到端验证，过程中发现并修复了若干真实的集成问题。完整规模的基准测试尚未完成，目前没有可公开的结果。完整流程和当前状态见 `benchmarks/massspecgym/README.md`。

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
