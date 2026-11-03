use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

const LATENCY_BUCKETS_MS: &[u64] = &[5, 10, 25, 50, 100, 250, 500, 1_000];

use recommendation_policy::metrics::RecommendationMetricsPort;
pub use recommendation_policy::metrics::{
    RecommendationEntity, RecommendationMetricOutcome, RecommendationMetricReason,
    RecommendationMetricSource, RecommendationMetricStage, RecommendationMetricSurface,
};

#[derive(Clone, Default)]
pub struct RecommendationMetrics {
    inner: Arc<Mutex<RecommendationMetricsState>>,
}

#[derive(Default)]
struct RecommendationMetricsState {
    counters: BTreeMap<String, i64>,
    histograms: BTreeMap<(RecommendationMetricSurfaceKey, RecommendationMetricStageKey), Histogram>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RecommendationMetricSurfaceKey(&'static str);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RecommendationMetricStageKey(&'static str);

#[derive(Clone, Debug, Default)]
struct Histogram {
    bucket_counts: Vec<u64>,
    count: u64,
    sum: u64,
}

impl RecommendationMetrics {
    pub fn record_request(
        &self,
        surface: RecommendationMetricSurface,
        outcome: RecommendationMetricOutcome,
    ) {
        self.inc(
            "recommendation_requests_total",
            &[("surface", surface.as_str()), ("outcome", outcome.as_str())],
            1,
        );
    }

    pub fn record_source_run(
        &self,
        surface: RecommendationMetricSurface,
        source: RecommendationMetricSource,
        outcome: RecommendationMetricOutcome,
    ) {
        self.inc(
            "recommendation_source_runs_total",
            &[
                ("surface", surface.as_str()),
                ("source", source.as_str()),
                ("outcome", outcome.as_str()),
            ],
            1,
        );
    }

    pub fn record_candidates(
        &self,
        surface: RecommendationMetricSurface,
        source: RecommendationMetricSource,
        outcome: RecommendationMetricOutcome,
        count: i64,
    ) {
        self.inc(
            "recommendation_candidates_total",
            &[
                ("surface", surface.as_str()),
                ("source", source.as_str()),
                ("outcome", outcome.as_str()),
            ],
            count.max(0),
        );
    }

    pub fn record_fallback(
        &self,
        surface: RecommendationMetricSurface,
        reason: RecommendationMetricReason,
    ) {
        self.inc(
            "recommendation_fallbacks_total",
            &[("surface", surface.as_str()), ("reason", reason.as_str())],
            1,
        );
    }

    pub fn record_refresh_run(
        &self,
        surface: RecommendationMetricSurface,
        outcome: RecommendationMetricOutcome,
    ) {
        self.inc(
            "recommendation_refresh_runs_total",
            &[("surface", surface.as_str()), ("outcome", outcome.as_str())],
            1,
        );
    }

    pub fn record_refresh_viewers(
        &self,
        surface: RecommendationMetricSurface,
        outcome: RecommendationMetricOutcome,
        count: i64,
    ) {
        self.inc(
            "recommendation_refresh_viewers_total",
            &[("surface", surface.as_str()), ("outcome", outcome.as_str())],
            count.max(0),
        );
    }

    pub fn record_retention_run(
        &self,
        surface: RecommendationMetricSurface,
        outcome: RecommendationMetricOutcome,
    ) {
        self.inc(
            "recommendation_retention_runs_total",
            &[("surface", surface.as_str()), ("outcome", outcome.as_str())],
            1,
        );
    }

    pub fn record_retention(
        &self,
        surface: RecommendationMetricSurface,
        entity: RecommendationEntity,
        outcome: RecommendationMetricOutcome,
        count: i64,
    ) {
        self.inc(
            "recommendation_retention_rows_total",
            &[
                ("surface", surface.as_str()),
                ("entity", entity.as_str()),
                ("outcome", outcome.as_str()),
            ],
            count.max(0),
        );
    }

    pub fn observe_stage_latency(
        &self,
        surface: RecommendationMetricSurface,
        stage: RecommendationMetricStage,
        millis: u64,
    ) {
        let mut state = self.inner.lock().expect("recommendation metrics");
        let histogram = state
            .histograms
            .entry((
                RecommendationMetricSurfaceKey(surface.as_str()),
                RecommendationMetricStageKey(stage.as_str()),
            ))
            .or_insert_with(|| Histogram {
                bucket_counts: vec![0; LATENCY_BUCKETS_MS.len() + 1],
                ..Histogram::default()
            });
        for (index, upper_bound) in LATENCY_BUCKETS_MS.iter().enumerate() {
            if millis <= *upper_bound {
                histogram.bucket_counts[index] += 1;
            }
        }
        histogram.bucket_counts[LATENCY_BUCKETS_MS.len()] += 1;
        histogram.count += 1;
        histogram.sum = histogram.sum.saturating_add(millis);
    }

    pub fn render_prometheus(&self) -> String {
        let state = self.inner.lock().expect("recommendation metrics");
        let mut output = String::new();
        for (sample, value) in &state.counters {
            output.push_str(sample);
            output.push(' ');
            output.push_str(&value.to_string());
            output.push('\n');
        }
        for ((surface, stage), histogram) in &state.histograms {
            for (index, upper_bound) in LATENCY_BUCKETS_MS.iter().enumerate() {
                output.push_str(&format!(
                    "recommendation_stage_latency_milliseconds_bucket{{surface=\"{}\",stage=\"{}\",le=\"{}\"}} {}\n",
                    surface.0, stage.0, upper_bound, histogram.bucket_counts[index]
                ));
            }
            output.push_str(&format!(
                "recommendation_stage_latency_milliseconds_bucket{{surface=\"{}\",stage=\"{}\",le=\"+Inf\"}} {}\n",
                surface.0,
                stage.0,
                histogram.bucket_counts[LATENCY_BUCKETS_MS.len()]
            ));
            output.push_str(&format!(
                "recommendation_stage_latency_milliseconds_count{{surface=\"{}\",stage=\"{}\"}} {}\n",
                surface.0, stage.0, histogram.count
            ));
            output.push_str(&format!(
                "recommendation_stage_latency_milliseconds_sum{{surface=\"{}\",stage=\"{}\"}} {}\n",
                surface.0, stage.0, histogram.sum
            ));
        }
        output
    }

    fn inc(&self, name: &str, labels: &[(&str, &str)], amount: i64) {
        let label_set = labels
            .iter()
            .map(|(key, value)| format!("{key}=\"{value}\""))
            .collect::<Vec<_>>()
            .join(",");
        let sample = format!("{name}{{{label_set}}}");
        let mut state = self.inner.lock().expect("recommendation metrics");
        *state.counters.entry(sample).or_insert(0) += amount;
    }
}

impl RecommendationMetricsPort for RecommendationMetrics {
    fn record_request(
        &self,
        surface: RecommendationMetricSurface,
        outcome: RecommendationMetricOutcome,
    ) {
        RecommendationMetrics::record_request(self, surface, outcome);
    }
    fn record_source_run(
        &self,
        surface: RecommendationMetricSurface,
        source: RecommendationMetricSource,
        outcome: RecommendationMetricOutcome,
    ) {
        RecommendationMetrics::record_source_run(self, surface, source, outcome);
    }
    fn record_candidates(
        &self,
        surface: RecommendationMetricSurface,
        source: RecommendationMetricSource,
        outcome: RecommendationMetricOutcome,
        count: i64,
    ) {
        RecommendationMetrics::record_candidates(self, surface, source, outcome, count);
    }
    fn record_fallback(
        &self,
        surface: RecommendationMetricSurface,
        reason: RecommendationMetricReason,
    ) {
        RecommendationMetrics::record_fallback(self, surface, reason);
    }
    fn observe_stage_latency(
        &self,
        surface: RecommendationMetricSurface,
        stage: RecommendationMetricStage,
        elapsed_ms: u64,
    ) {
        RecommendationMetrics::observe_stage_latency(self, surface, stage, elapsed_ms);
    }
}
