#[cfg(feature = "plot")]
pub mod svg {
    use std::path::Path;

    use anyhow::Context;
    use masstrust_core::RiskCoverageRow;
    use plotters::prelude::*;

    pub fn render(
        curve: &[RiskCoverageRow],
        path: &Path,
        target_risk: Option<f64>,
    ) -> anyhow::Result<()> {
        let root = SVGBackend::new(path, (800, 500)).into_drawing_area();
        root.fill(&WHITE).context("fill background")?;

        let mut chart = ChartBuilder::on(&root)
            .caption("Risk-Coverage Curve", ("sans-serif", 18))
            .margin(30)
            .x_label_area_size(40)
            .y_label_area_size(50)
            .build_cartesian_2d(0.0f64..1.0, 0.0f64..1.0)
            .context("build chart")?;

        chart
            .configure_mesh()
            .x_desc("Coverage")
            .y_desc("Risk")
            .draw()
            .context("draw mesh")?;

        let points: Vec<(f64, f64)> = std::iter::once((0.0, 0.0))
            .chain(curve.iter().map(|r| (r.coverage, r.risk.unwrap_or(0.0))))
            .collect();

        chart
            .draw_series(LineSeries::new(points, BLUE.stroke_width(2)))
            .context("draw curve")?
            .label("risk-coverage")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLUE.stroke_width(2)));

        if let Some(t) = target_risk {
            chart
                .draw_series(LineSeries::new(
                    vec![(0.0, t), (1.0, t)],
                    RED.stroke_width(1),
                ))
                .context("draw target line")?
                .label(format!("target risk {t:.3}"))
                .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED.stroke_width(1)));
        }

        chart
            .configure_series_labels()
            .background_style(WHITE)
            .border_style(BLACK)
            .draw()
            .context("draw legend")?;

        root.present().context("write SVG")?;
        Ok(())
    }

    pub fn histogram(confidences: &[Option<f64>], path: &Path) -> anyhow::Result<()> {
        const N_BINS: usize = 20;

        let values: Vec<f64> = confidences.iter().filter_map(|&c| c).collect();
        if values.is_empty() {
            anyhow::bail!("No scoreable queries for histogram");
        }

        let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        // Ensure a non-zero range so all values fall in the first bin gracefully
        let range = (max_val - min_val).max(1e-10);
        let bin_width = range / N_BINS as f64;

        let mut counts = [0usize; N_BINS];
        for &v in &values {
            let bin = ((v - min_val) / range * N_BINS as f64).floor() as usize;
            counts[bin.min(N_BINS - 1)] += 1;
        }

        let max_count = *counts.iter().max().unwrap_or(&1);

        let root = SVGBackend::new(path, (800, 400)).into_drawing_area();
        root.fill(&WHITE).context("fill background")?;

        let mut chart = ChartBuilder::on(&root)
            .caption("Confidence Score Distribution", ("sans-serif", 18))
            .margin(30)
            .x_label_area_size(40)
            .y_label_area_size(50)
            .build_cartesian_2d(min_val..max_val, 0usize..max_count + 1)
            .context("build chart")?;

        chart
            .configure_mesh()
            .x_desc("Confidence")
            .y_desc("Count")
            .draw()
            .context("draw mesh")?;

        chart
            .draw_series(counts.iter().enumerate().map(|(i, &count)| {
                let x0 = min_val + i as f64 * bin_width;
                let x1 = x0 + bin_width * 0.9;
                Rectangle::new([(x0, 0_usize), (x1, count)], BLUE.filled())
            }))
            .context("draw bars")?;

        root.present().context("write SVG")?;
        Ok(())
    }
}
