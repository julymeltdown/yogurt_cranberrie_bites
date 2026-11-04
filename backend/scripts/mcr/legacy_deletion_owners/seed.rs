//! Populated privacy fixtures. Only typed generated UUIDs enter fixture SQL.
#![allow(dead_code)]
use sqlx::PgPool;
use uuid::Uuid;
pub async fn populate(pool: &PgPool, owner: &str, user: Uuid) -> anyhow::Result<Uuid> {
    let peer = Uuid::new_v4();
    let other = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    let d = Uuid::new_v4();
    let sql = match owner {
        "profile" => format!(
            r#"
          insert into profiles(user_id,username,nickname,avatar_url,bio,follower_count) values
            ('{user}','u-{user}','private nickname','private://avatar','private bio',7),
            ('{peer}','u-{peer}','peer nickname','peer://avatar','peer bio',11);
          insert into avatars(user_id,content,content_type,filename) values('{user}',decode('010203','hex'),'image/png','private.png');
          insert into profile_mutation_receipts(operation_id,actor_id,scope,command_fingerprint,outcome_json)
            values('{a}','{user}','profile.v1.UpdateProfile','fixture','{{"bio":"private bio"}}');
        "#
        ),
        "friend" => format!(
            r#"
          insert into follows(follower_id,followee_id) values('{user}','{peer}'),('{peer}','{user}'),('{peer}','{other}');
          insert into follow_requests(id,follower_id,followee_id,status,created_at) values('{a}','{user}','{peer}','PENDING',now());
          insert into user_blocks(blocker_id,blocked_id,created_at) values('{user}','{peer}',now());
          insert into user_mutes(muter_id,muted_id,created_at) values('{peer}','{user}',now());
          insert into people_recommendation_snapshots(id,viewer_id,rank_version,created_at,expires_at)
            values('{b}','{peer}','fixture',now(),now()+interval '1 hour'),('{c}','{user}','fixture',now(),now()+interval '1 hour');
          insert into people_recommendation_snapshot_items(snapshot_id,candidate_user_id,source,reason_code,score_micros,rank_version,generated_at)
            values('{b}','{user}','fixture','fixture',1,'fixture',now()),('{b}','{other}','fixture','fixture',1,'fixture',now());
        "#
        ),
        "post" => format!(
            r#"
          insert into posts(id,author_id,content,visibility,created_at,comment_count,like_count) values
            ('{a}','{user}','private post','PUBLIC',now(),0,0),('{b}','{peer}','peer post','PUBLIC',now(),1,1);
          insert into comments(id,post_id,author_id,content,created_at) values('{c}','{b}','{user}','private comment',now());
          insert into post_likes(post_id,user_id,created_at) values('{b}','{user}',now());
          insert into post_event_outbox(event_id,event_type,aggregate_id,actor_id,payload,occurred_at,created_at)
            values('{a}','POST_CREATED','{a}','{user}','{{"postId":"{a}","authorId":"{user}","content":"private outbox body"}}',now(),now());
          insert into event_publications(event_id,target,published_at,payload)
            values('{a}','INTERNAL',now(),'{{"postId":"{a}","authorId":"{user}","content":"private publication body"}}');
          insert into post_mutation_receipts(operation_id,actor_id,scope,command_fingerprint,outcome_json)
            values('{d}','{user}','post.v1.CreatePost','fixture','{{"content":"private post"}}');
          insert into post_concert_posts(post_id,event_id,author_id,content,revision,state,created_at,comment_count,reaction_count) values
            ('{a}','{other}','{user}','{{"text":"private concert","media_ids":[],"links":[]}}',1,'PUBLISHED',now(),0,0),
            ('{b}','{other}','{peer}','{{"text":"peer concert","media_ids":[],"links":[]}}',1,'PUBLISHED',now(),1,1);
          insert into post_concert_comments(comment_id,event_id,post_id,author_id,text,revision,state,created_at)
            values('{c}','{other}','{b}','{user}','private concert comment',1,'PUBLISHED',now());
          insert into post_concert_reactions(event_id,post_id,actor_id,created_at) values('{other}','{b}','{user}',now());
        "#
        ),
        _ => anyhow::bail!("Unknown fixture owner"),
    };
    sqlx::raw_sql(&sql).execute(pool).await?;
    Ok(peer)
}

pub async fn assert_peer_survives(pool: &PgPool, owner: &str, peer: Uuid) -> anyhow::Result<()> {
    match owner {
        "profile" => {
            let actual: (String,String,i64) = sqlx::query_as("select nickname,bio,follower_count from profiles where user_id=$1 and deleted_at is null").bind(peer).fetch_one(pool).await?;
            assert_eq!(actual, ("peer nickname".into(), "peer bio".into(), 11));
        }
        "friend" => {
            let n: i64 = sqlx::query_scalar(
                "select count(*) from follows where follower_id=$1 and deleted_at is null",
            )
            .bind(peer)
            .fetch_one(pool)
            .await?;
            assert_eq!(n, 1);
            let n:i64=sqlx::query_scalar("select count(*) from people_recommendation_snapshot_items i join people_recommendation_snapshots s on s.id=i.snapshot_id where s.viewer_id=$1").bind(peer).fetch_one(pool).await?;
            assert_eq!(n, 1);
        }
        "post" => {
            let actual:(String,i64,i64)=sqlx::query_as("select content,comment_count,like_count from posts where author_id=$1 and deleted_at is null").bind(peer).fetch_one(pool).await?;
            assert_eq!(actual, ("peer post".into(), 0, 0));
            let actual:(i64,i64,i64)=sqlx::query_as("select comment_count,reaction_count,revision from post_concert_posts where author_id=$1 and state='PUBLISHED'").bind(peer).fetch_one(pool).await?;
            assert_eq!(actual, (0, 0, 2));
        }
        _ => anyhow::bail!("Unknown fixture owner"),
    }
    Ok(())
}
