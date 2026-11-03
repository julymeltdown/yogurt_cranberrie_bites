//! Audited contact detector adapted from Bigyeting 5eecdcaf social_intro_contact_policy.rs.
//! Source literals retained; no source fallback UI text or unrelated fields.
pub fn contact_bypass(value: &str) -> bool {
    let normalized = fold_contact_text(value);
    let compact = normalized
        .chars()
        .filter(|character| {
            !character.is_whitespace()
                && !matches!(
                    character,
                    '[' | ']' | '(' | ')' | '{' | '}' | '\u{200b}' | '\u{200c}' | '\u{200d}'
                )
        })
        .collect::<String>();
    let alphanumeric = normalized
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect::<String>();
    let link_or_handle = normalized.contains('@')
        || compact.contains("http://")
        || compact.contains("https://")
        || domain(&compact, "instagram.com")
        || domain(&compact, "twitter.com")
        || domain(&compact, "x.com")
        || domain(&compact, "discord.gg")
        || domain(&compact, "t.me")
        || alphanumeric.contains("instagramdotcom")
        || alphanumeric.contains("discorddotgg")
        || alphanumeric.contains("tdotme")
        || normalized.contains("kakao")
        || normalized.contains("카카오톡")
        || normalized.contains("오픈채팅")
        || line_identifier(&normalized, &compact);
    let digit_count = normalized.chars().filter(char::is_ascii_digit).count();
    link_or_handle || digit_count >= 9
}

fn fold_contact_text(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| {
            let folded = match character {
                '\u{ff01}'..='\u{ff5e}' => {
                    char::from_u32(character as u32 - 0xfee0).map_or(character, |value| value)
                }
                '\u{3002}' | '\u{ff61}' => '.',
                '\u{2215}' | '\u{2044}' => '/',
                other => other,
            };
            folded.to_lowercase()
        })
        .collect()
}

fn domain(value: &str, candidate: &str) -> bool {
    value.match_indices(candidate).any(|(index, _)| {
        let before = value[..index].chars().next_back();
        let after = value[index + candidate.len()..].chars().next();
        before.is_none_or(|character| !character.is_alphanumeric())
            && after.is_none_or(|character| !character.is_alphanumeric())
    })
}

fn line_identifier(normalized: &str, compact: &str) -> bool {
    compact.contains("line:id")
        || compact.contains("line_id")
        || compact.contains("line-id")
        || normalized.contains("line id")
        || normalized.contains("line 아이디")
        || normalized.contains("라인 id")
        || normalized.contains("라인아이디")
}
