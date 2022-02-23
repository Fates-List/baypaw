// A *very* primitive and possibly insecure IPC implementation 
// that will also hold the admin bot (in serenity) soon
//
// This should never be run without a firewall blocking all remote 
// requests to port 1234!
use actix_web::{web, get, App, HttpRequest, HttpServer, Responder, HttpResponse};
use serde::{Deserialize, Serialize};
use log::debug;
mod database;
use std;
use env_logger;

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
    })
    .bind(("127.0.0.1", 1234))
    .unwrap()
    .run()
    .await
    .unwrap();

    Ok(())
}
