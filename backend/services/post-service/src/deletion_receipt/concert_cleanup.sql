-- One statement, inside the original subject-fenced owner transaction.
-- Metadata-only operation receipts/outbox remain replay fences; they contain no
-- post/comment bodies or media. Never fabricate publication acknowledgements.
with affected as materialized (
    select post_id from post_concert_posts where author_id=$1
    union select post_id from post_concert_comments where author_id=$1
    union select post_id from post_concert_reactions where actor_id=$1
), locked as materialized (
    select p.post_id from post_concert_posts p join affected using(post_id)
    order by p.post_id for update of p
), removed_comments as (
    update post_concert_comments c
    set text='[deleted]',state='DELETED',revision=c.revision+1,edited_at=now()
    where c.author_id=$1 and (c.state<>'DELETED' or c.text<>'[deleted]')
      and c.post_id in (select post_id from locked)
    returning c.post_id, (select state from post_concert_comments old where old.comment_id=c.comment_id) as old_state
), removed_reactions as (
    delete from post_concert_reactions r where r.actor_id=$1
      and r.post_id in (select post_id from locked) returning r.post_id
)
update post_concert_posts p
set content=case when p.author_id=$1 then '{"text":"[deleted]","media_ids":[],"links":[]}'::jsonb else p.content end,
    state=case when p.author_id=$1 then 'DELETED' else p.state end,
    revision=p.revision+1,edited_at=now(),
    comment_count=greatest(0,p.comment_count-(select count(*) from removed_comments c where c.post_id=p.post_id and c.old_state='PUBLISHED')),
    reaction_count=greatest(0,p.reaction_count-(select count(*) from removed_reactions r where r.post_id=p.post_id))
where p.post_id in (select post_id from locked);
