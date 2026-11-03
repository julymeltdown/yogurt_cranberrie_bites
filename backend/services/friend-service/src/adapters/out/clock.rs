use crate::application::ports::PeopleRecommendationClockPort;
use chrono::{DateTime, Utc};

#[derive(Clone, Copy, Default)]
pub struct SystemPeopleRecommendationClock;

impl PeopleRecommendationClockPort for SystemPeopleRecommendationClock {
    fn now_utc(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
