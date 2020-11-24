use crate::db;
use crate::error::ApiError;
use crate::schema::guilds;
use diesel::prelude::*;

#[derive(Debug, Serialize, Queryable, Deserialize, Insertable)]
#[table_name = "guilds"]
pub struct Guilds {
    pub id: i32,
    pub guild_id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub locale: String,
}

#[derive(Debug, Serialize, Deserialize, AsChangeset, Insertable)]
#[table_name = "guilds"]
pub struct GuildsMessage {
    pub guild_id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub locale: String,
}

impl Guilds {
    pub fn find_by_id(id: i32) -> Result<Self, ApiError> {
        let conn = db::connection()?;
        let guild = guilds::table.filter(guilds::id.eq(id)).first(&conn)?;
        Ok(guild)
    }

    pub fn find_by_guild_id(guild_id: String) -> Result<Self, ApiError> {
        let conn = db::connection()?;
        let guild = guilds::table.filter(guilds::guild_id.eq(guild_id)).first(&conn)?;
        Ok(guild)
    }

    pub fn joined(guild_ids: Vec<String>) -> Result<Vec<Guilds>, ApiError> {
        let conn = db::connection()?;
        use crate::schema::guilds::dsl::*;
        let guild = guilds.filter(guild_id.eq_any(guild_ids)).get_results::<Guilds>(&conn)?;
        Ok(guild)
    }

    pub fn create(guild: GuildsMessage) -> Result<Self, ApiError> {
        let conn = db::connection()?;
        let guild = diesel::insert_into(guilds::table)
                            .values(guild)
                            .get_result(&conn)?;
        Ok(guild)
    }

    pub fn update(id: i32, guild: GuildsMessage) -> Result<Self, ApiError> {
        let conn = db::connection()?;
        let guild = diesel::update(guilds::table.find(id))
                            .set(guild)
                            .get_result(&conn)?;
        Ok(guild)
    }

    pub fn delete(id: i32) -> Result<usize, ApiError> {
        let conn = db::connection()?;
        let size = diesel::delete(guilds::table.find(id))
                            .execute(&conn)?;
        Ok(size)
    }
}
