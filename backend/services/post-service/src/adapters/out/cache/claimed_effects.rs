//! Required deadline-aware effects. A queued Redis command checks server TIME
//! before the atomic mutation, even when the caller has already timed out.
use super::{
    connect_manager,
    feed_cache::{feed_cache_generation_key_for_epoch, FEED_CACHE_GENERATION_TTL_SECONDS},
    post_cache::RedisPostCache,
};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use post_application::{
    cache_authority::ClaimedCacheEffectsPort,
    delivery_effect::{DeliveryEffectDeadlineDto, DeliveryEffectFailure},
};
use redis::aio::ConnectionManager;
use std::time::Duration;
use uuid::Uuid;

// Decimal strings avoid loss of timestamp precision in Lua numbers. One atomic
// server-entry check fences the bounded mutation; there is no rollback claim
// once script dispatch starts or its acknowledgement is lost.
const MUTATE_UNTIL: &str = r#"
local t=redis.call('TIME')
local now=t[1]..string.format('%06d',tonumber(t[2]))
local stop=ARGV[1]
if #now>#stop or (#now==#stop and now>=stop) then return 0 end
if ARGV[2]=='POST' then
  redis.call('DEL',KEYS[1])
else
  for _,key in ipairs(KEYS) do
    redis.call('INCR',key)
    redis.call('EXPIRE',key,ARGV[3])
  end
end
return 1
"#;
#[derive(Clone)]
pub struct RedisClaimedCacheEffects {
    connection: ConnectionManager,
    epoch: u64,
}
impl RedisClaimedCacheEffects {
    pub async fn new(client: redis::Client, epoch: u64) -> anyhow::Result<Self> {
        anyhow::ensure!(epoch > 0, "Invalid cache namespace epoch");
        Ok(Self {
            connection: connect_manager(client).await?,
            epoch,
        })
    }
    async fn mutate(
        &self,
        keys: Vec<String>,
        mode: &str,
        deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<()> {
        let micros = deadline
            .expires_at()
            .timestamp_micros()
            .saturating_sub(chrono::Utc::now().timestamp_micros());
        let ready = (|| -> anyhow::Result<_> {
            anyhow::ensure!(micros > 0, "Cache authority expired");
            let d = AuthorityRpcDeadline::within_original(
                deadline.expires_at().timestamp_micros(),
                deadline.monotonic_stop(),
                Duration::from_micros(micros as u64),
            )?;
            d.remaining(Duration::from_secs(60), d.expires_micros())?;
            Ok(d)
        })()
        .map_err(DeliveryEffectFailure::confirmed)?;
        // Redis cannot observe this process's monotonic clock. Cap its absolute
        // wall deadline by the remaining original monotonic budget at dispatch.
        let server_expiry = deadline.expires_at().timestamp_micros().min(
            chrono::Utc::now().timestamp_micros().saturating_add(
                ready
                    .remaining(Duration::from_secs(86_400), ready.expires_micros())
                    .map_err(|e| DeliveryEffectFailure::confirmed(e.into()))?
                    .as_micros() as i64,
            ),
        );
        let operation = async {
            let mut connection = self.connection.clone();
            let applied: i64 = redis::cmd("EVAL")
                .arg(MUTATE_UNTIL)
                .arg(keys.len())
                .arg(&keys)
                .arg(server_expiry)
                .arg(mode)
                .arg(FEED_CACHE_GENERATION_TTL_SECONDS)
                .query_async(&mut connection)
                .await?;
            if applied == 0 {
                return Err(DeliveryEffectFailure::confirmed(anyhow::anyhow!(
                    "Cache authority expired before server mutation"
                )));
            }
            anyhow::ensure!(applied == 1, "Invalid cache mutation acknowledgement");
            ready.remaining(Duration::from_secs(60), ready.expires_micros())?;
            Ok(())
        };
        match tokio::time::timeout_at(ready.stop(), operation).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error))
                if error.downcast_ref::<DeliveryEffectFailure>()
                    == Some(&DeliveryEffectFailure::ConfirmedNoEffect) =>
            {
                Err(error)
            }
            Ok(Err(error)) => Err(DeliveryEffectFailure::uncertain(error)),
            Err(error) => Err(DeliveryEffectFailure::uncertain(error.into())),
        }
    }
}
#[async_trait::async_trait]
impl ClaimedCacheEffectsPort for RedisClaimedCacheEffects {
    async fn evict_post(
        &self,
        post_id: Uuid,
        deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(!post_id.is_nil(), "Invalid cache post");
        self.mutate(vec![RedisPostCache::key(post_id)], "POST", deadline)
            .await
    }
    async fn invalidate_feeds(
        &self,
        user_ids: &[Uuid],
        deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            !user_ids.is_empty() && user_ids.len() <= 101 && user_ids.iter().all(|id| !id.is_nil()),
            "Invalid cache recipient batch"
        );
        self.mutate(
            user_ids
                .iter()
                .map(|id| feed_cache_generation_key_for_epoch(self.epoch, *id))
                .collect(),
            "FEED",
            deadline,
        )
        .await
    }
}
