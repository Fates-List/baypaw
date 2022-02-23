// A *very* primitive and possibly insecure IPC implementation 
// that will also hold the admin bot (in serenity) soon
//
// This should never be run without a firewall blocking all remote 
// requests to port 1234!
use actix_web::{web, get, post, App, HttpRequest, HttpServer, Responder, HttpResponse};
use serde::{Deserialize, Serialize};
use log::{debug, error};
mod database;
use std;
use env_logger;
use serenity::model::prelude::*;
use serde_json::json;

#[derive(Serialize, Deserialize)]
struct User {
    username: String,
    disc: String,
    id: String,
    avatar: String,
    bot: bool,
}

#[get("/getch/{id}")]
async fn getch(req: HttpRequest, id: web::Path<u64>) -> HttpResponse {
    let data: &IpcAppData = req.app_data::<web::Data<IpcAppData>>().unwrap();

    let user = data.database.getch(id.into_inner()).await;

    if user.is_some() {
        let user = user.unwrap();
        debug!("Found user {}", user);

        let avatar = user.avatar_url().unwrap_or_else(|| "".to_string());

        return HttpResponse::Ok().json(User {
            username: user.name,
            disc: user.discriminator.to_string(),
            id: user.id.to_string(),
            avatar: avatar,
            bot: user.bot,
        });
    }
    HttpResponse::NotFound().finish()
}

#[derive(Serialize, Deserialize)]
struct Message {
    pub channel_id: u64,
    pub content: String,
    pub embed: serenity::model::channel::Embed,
    pub mention_roles: Vec<String>,
}

#[post("/messages")]
async fn send_message(req: HttpRequest, msg: web::Json<Message>) -> HttpResponse {
    let data: &IpcAppData = req.app_data::<web::Data<IpcAppData>>().unwrap();

    let res = data.database.clis.main.http.send_message(msg.channel_id, &json!({
        "content": msg.content,
        "embeds": vec![msg.embed.clone()],
        "mention_roles": msg.mention_roles,
    })).await;

    if res.is_err() {
        error!("Error sending message: {:?}", res.err());
        return HttpResponse::BadRequest().finish();
    }

    HttpResponse::Ok().finish()
}

struct IpcAppData {
    database: database::Database,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "baypaw=debug,actix_web=info");

    env_logger::init();

    let database = database::Database::new().await;
    let app_data = web::Data::new(IpcAppData {
        database,
    });

    HttpServer::new(move || {
        App::new()
            .app_data(app_data.clone())
            .wrap(actix_web::middleware::Logger::default())
            .service(getch)
            .service(send_message)
    })
    .bind(("127.0.0.1", 1234))
    .unwrap()
    .run()
    .await
    .unwrap();

    Ok(())
}
