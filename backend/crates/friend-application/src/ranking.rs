use crate::ports::PeopleRankerPort;
use anyhow::Result;
use friend_domain::recommendation::{
    rank_people_candidates, PeopleCandidate, PeopleRankWeights, RankedPeopleCandidate,
};

#[derive(Debug, Clone)]
pub struct LinearPeopleRanker {
    weights: PeopleRankWeights,
}

impl LinearPeopleRanker {
    pub fn new(weights: PeopleRankWeights) -> Self {
        Self { weights }
    }
}

impl PeopleRankerPort for LinearPeopleRanker {
    fn version(&self) -> &str {
        &self.weights.version
    }

    fn rank(&self, candidates: Vec<PeopleCandidate>) -> Result<Vec<RankedPeopleCandidate>> {
        Ok(rank_people_candidates(candidates, &self.weights)?)
    }
}
