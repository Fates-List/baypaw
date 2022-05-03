#![feature(derive_default_enum)]

// A *very* primitive and possibly insecure IPC implementation 
// that will also hold the admin bot (in serenity) soon
//
// This should never be run without a firewall blocking all remote 
// requests to port 1234!
use actix_web::{web, get, post, App, HttpRequest, HttpServer, HttpResponse};
use serde::{Deserialize, Serialize};
use log::{debug, error};
mod database;
use std;
use env_logger;
use serenity::model::id::GuildId;
use serde_json::json;
use bristlefrost::models::{User, Status};

#[get("/perms/{id}")]
async fn user_perms(req: HttpRequest, id: web::Path<u64>) -> HttpResponse {
    let data: &IpcAppData = req.app_data::<web::Data<IpcAppData>>().unwrap();

    HttpResponse::Ok().json(data.database.get_user_perms(id.into_inner()).await)
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
            status: Status::Unknown,
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

/// Important: This API does not handle server privacy. This should be 
/// done server-side

#[derive(Serialize, Deserialize)]
struct GuildInviteQuery {
    cid: u64, // Channel ID
    uid: u64, // User ID
    gid: u64, // Guild ID
}

#[derive(Serialize, Deserialize)]
struct GuildInviteData {
    url: String,
    cid: u64, // First successful cid
}

#[get("/guild-invite")]
async fn guild_invite(req: HttpRequest, info: web::Query<GuildInviteQuery>) -> HttpResponse {
    let data: &IpcAppData = req.app_data::<web::Data<IpcAppData>>().unwrap();

    if info.cid.clone() != 0 {
        let invite_code = data.database.guild_invite(info.cid, info.uid).await;

        if let Some(url) = invite_code {
            return HttpResponse::Ok().json(GuildInviteData {
                url: url,
                cid: info.cid.clone(),
            });
        }
    }
    
    // First get channels from cache
    let chan_cache = GuildId(info.gid).to_guild_cached(data.database.clis.servers.cache.clone());

    if let Some(guild) = chan_cache {
        let channels = guild.channels;
        for channel in channels.keys() {
            let invite_code = data.database.guild_invite(channel.0, info.uid).await;

            if let Some(url) = invite_code {
                return HttpResponse::Ok().json(GuildInviteData {
                    url: url,
                    cid: u64::from(channel.0),
                });
            }
        }
    } else {
        let res = GuildId(info.gid).channels(data.database.clis.servers.http.clone()).await;
        if let Err(err) = res {
            error!("Error getting channels: {:?}", err);
            return HttpResponse::BadRequest().finish();
        }
        let channels = res.unwrap();
        for channel in channels.keys() {
            let invite_code = data.database.guild_invite(channel.0, info.uid).await;

            if let Some(url) = invite_code {
                return HttpResponse::Ok().json(GuildInviteData {
                    url: url,
                    cid: u64::from(channel.0),
                });
            }
        }
    }
    debug!("Failed to fetch guild");
    HttpResponse::NotFound().finish()
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
            .service(user_perms)
            .service(getch)
            .service(send_message)
            .service(guild_invite)
    })
    .workers(6)
    .bind(("127.0.0.1", 1234))
    .unwrap()
    .run()
    .await
    .unwrap();

    Ok(())
}
