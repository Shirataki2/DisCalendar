table! {
    event_settings (id) {
        id -> Int4,
        guild_id -> Text,
        channel_id -> Text,
    }
}

table! {
    events (id) {
        id -> Int4,
        guild_id -> Text,
        name -> Text,
        description -> Nullable<Text>,
        notifications -> Array<Text>,
        color -> Text,
        is_all_day -> Bool,
        start_at -> Timestamp,
        end_at -> Timestamp,
        created_at -> Timestamp,
    }
}

table! {
    guilds (id) {
        id -> Int4,
        guild_id -> Text,
        name -> Text,
        avatar_url -> Nullable<Text>,
        locale -> Text,
    }
}

allow_tables_to_appear_in_same_query!(
    event_settings,
    events,
    guilds,
);
