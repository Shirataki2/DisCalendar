use crate::db;
use crate::error::ApiError;
use crate::schema::{events, event_settings};
use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Debug, Serialize, Queryable, Deserialize, Insertable)]
#[table_name = "events"]
pub struct Events {
    pub id: i32,
    pub guild_id: String,
    pub name: String,
    pub description: Option<String>,
    pub notifications: Vec<String>,
    pub color: String,
    pub is_all_day: bool,
    pub start_at: NaiveDateTime,
    pub end_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, AsChangeset, Insertable)]
#[table_name = "events"]
pub struct EventsMessage {
    pub name: String,
    pub description: Option<String>,
    pub notifications: Vec<String>,
    pub color: String,
    pub is_all_day: bool,
    pub start_at: NaiveDateTime,
    pub end_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, AsChangeset, Insertable)]
#[table_name = "events"]
pub struct EventCreate {
    pub guild_id: String,
    pub name: String,
    pub description: Option<String>,
    pub notifications: Vec<String>,
    pub color: String,
    pub is_all_day: bool,
    pub start_at: NaiveDateTime,
    pub end_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Queryable, Deserialize, Insertable)]
#[table_name = "event_settings"]
pub struct EventSettings {
    pub id: i32,
    pub guild_id: String,
    pub channel_id: String,
}

impl Events {
    pub fn find_by_id(id: i32) -> Result<Self, ApiError> {
        let conn = db::connection()?;
        let event = events::table.filter(events::id.eq(id)).first(&conn)?;
        Ok(event)
    }

    pub fn find_by_guild_id_with_duration(guild_id: String, start_at: NaiveDateTime, end_at: NaiveDateTime) -> Result<Vec<Self>, ApiError> {
        let conn = db::connection()?;
        let events = events::table
            .filter(events::guild_id.eq(guild_id))
            .filter(events::start_at.gt(start_at))
            .filter(events::start_at.lt(end_at))
            .get_results::<Events>(&conn)?;
        Ok(events)
    }

    pub fn find_by_guild_id(guild_id: String) -> Result<Vec<Self>, ApiError> {
        let conn = db::connection()?;
        let events = events::table.filter(events::guild_id.eq(guild_id)).get_results::<Events>(&conn)?;
        Ok(events)
    }

    pub fn create(event: EventCreate) -> Result<Self, ApiError> {
        let conn = db::connection()?;
        let event = diesel::insert_into(events::table)
                            .values(event)
                            .get_result(&conn)?;
        Ok(event)
    }

    pub fn update(id: i32, event: EventsMessage) -> Result<Self, ApiError> {
        let conn = db::connection()?;
        let event = diesel::update(events::table.find(id))
                            .set(event)
                            .get_result(&conn)?;
        Ok(event)
    }

    pub fn delete(id: i32) -> Result<usize, ApiError> {
        let conn = db::connection()?;
        let size = diesel::delete(events::table.find(id))
                            .execute(&conn)?;
        Ok(size)
    }
}
