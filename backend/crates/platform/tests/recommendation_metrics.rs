use platform::recommendation_metrics::{
    RecommendationEntity, RecommendationMetricOutcome, RecommendationMetricReason,
    RecommendationMetricSource, RecommendationMetricStage, RecommendationMetricSurface,
    RecommendationMetrics,
};

#[test]
fn recommendation_metrics_render_only_bounded_enum_labels_and_histogram_buckets() {
    let metrics = RecommendationMetrics::default();
    metrics.record_request(
        RecommendationMetricSurface::People,
        RecommendationMetricOutcome::Success,
    );
    metrics.record_candidates(
        RecommendationMetricSurface::People,
        RecommendationMetricSource::MutualGraph,
        RecommendationMetricOutcome::Success,
        17,
    );
    metrics.record_source_run(
        RecommendationMetricSurface::HomeFeed,
        RecommendationMetricSource::SocialProof,
        RecommendationMetricOutcome::Failure,
    );
    metrics.record_fallback(
        RecommendationMetricSurface::HomeFeed,
        RecommendationMetricReason::ModelUnavailable,
    );
    metrics.record_refresh_run(
        RecommendationMetricSurface::People,
        RecommendationMetricOutcome::Success,
    );
    metrics.record_refresh_viewers(
        RecommendationMetricSurface::People,
        RecommendationMetricOutcome::Success,
        4,
    );
    metrics.record_retention_run(
        RecommendationMetricSurface::HomeFeed,
        RecommendationMetricOutcome::Success,
    );
    metrics.record_retention(
        RecommendationMetricSurface::HomeFeed,
        RecommendationEntity::Snapshot,
        RecommendationMetricOutcome::Success,
        3,
    );
    metrics.record_retention(
        RecommendationMetricSurface::HomeFeed,
        RecommendationEntity::SnapshotItem,
        RecommendationMetricOutcome::Success,
        5,
    );
    metrics.observe_stage_latency(
        RecommendationMetricSurface::HomeFeed,
        RecommendationMetricStage::Ranking,
        42,
    );

    let rendered = metrics.render_prometheus();

    assert!(rendered
        .contains("recommendation_requests_total{surface=\"people\",outcome=\"success\"} 1\n"));
    assert!(rendered.contains(
        "recommendation_candidates_total{surface=\"people\",source=\"mutual_graph\",outcome=\"success\"} 17\n"
    ));
    assert!(rendered.contains(
        "recommendation_source_runs_total{surface=\"home_feed\",source=\"social_proof\",outcome=\"failure\"} 1\n"
    ));
    assert!(rendered.contains(
        "recommendation_fallbacks_total{surface=\"home_feed\",reason=\"model_unavailable\"} 1\n"
    ));
    assert!(rendered
        .contains("recommendation_refresh_runs_total{surface=\"people\",outcome=\"success\"} 1\n"));
    assert!(rendered.contains(
        "recommendation_refresh_viewers_total{surface=\"people\",outcome=\"success\"} 4\n"
    ));
    assert!(rendered.contains(
        "recommendation_retention_runs_total{surface=\"home_feed\",outcome=\"success\"} 1\n"
    ));
    assert!(rendered.contains(
        "recommendation_retention_rows_total{surface=\"home_feed\",entity=\"snapshot\",outcome=\"success\"} 3\n"
    ));
    assert!(rendered.contains(
        "recommendation_retention_rows_total{surface=\"home_feed\",entity=\"snapshot_item\",outcome=\"success\"} 5\n"
    ));
    assert!(rendered.contains(
        "recommendation_stage_latency_milliseconds_bucket{surface=\"home_feed\",stage=\"ranking\",le=\"50\"} 1\n"
    ));
    assert!(rendered.contains(
        "recommendation_stage_latency_milliseconds_count{surface=\"home_feed\",stage=\"ranking\"} 1\n"
    ));
    assert!(rendered.contains(
        "recommendation_stage_latency_milliseconds_sum{surface=\"home_feed\",stage=\"ranking\"} 42\n"
    ));
    assert!(!rendered.contains("11111111-1111-1111-1111-111111111111"));
}

#[test]
fn latency_observations_are_cumulative_across_fixed_buckets() {
    let metrics = RecommendationMetrics::default();
    for millis in [4, 12, 90, 1_500] {
        metrics.observe_stage_latency(
            RecommendationMetricSurface::People,
            RecommendationMetricStage::CandidateCollection,
            millis,
        );
    }

    let rendered = metrics.render_prometheus();

    assert!(rendered.contains(
        "recommendation_stage_latency_milliseconds_bucket{surface=\"people\",stage=\"candidate_collection\",le=\"5\"} 1\n"
    ));
    assert!(rendered.contains(
        "recommendation_stage_latency_milliseconds_bucket{surface=\"people\",stage=\"candidate_collection\",le=\"100\"} 3\n"
    ));
    assert!(rendered.contains(
        "recommendation_stage_latency_milliseconds_bucket{surface=\"people\",stage=\"candidate_collection\",le=\"+Inf\"} 4\n"
    ));
}
