table! {
    guilds (id) {
        id -> Int4,
        guild_id -> Text,
        name -> Text,
        avatar_url -> Nullable<Text>,
        locale -> Text,
    }
}
