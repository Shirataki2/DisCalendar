#[macro_use]
extern crate log;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

mod schema;
mod error;
mod db;
mod client;
mod http;
mod guilds;

use actix_web::{App, HttpServer, get};
use actix_web::middleware::Logger;

use std::env;

use dotenv::dotenv;

#[get("/")]
async fn index() -> String {
    "Hello World".to_string()
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    env_logger::init();
    db::init();

    let host = env::var("APP_HOST").unwrap_or("0.0.0.0".to_string());
    let port = env::var("APP_PORT").unwrap_or("5000".to_string());
    let server = HttpServer::new(|| {
        App::new()
            .service(index)
            .configure(guilds::router::init_routes)
            .wrap(Logger::default())  
    })
    .bind(format!("{}:{}", host, port))?;
    info!("Serving at {}:{}", host, port);
    server.run().await
}
