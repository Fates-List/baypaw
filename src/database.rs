use deadpool_redis::{Config, Runtime};
use std::path::PathBuf;
use std::env;
use serde::{Serialize, Deserialize};
use serenity::{
    prelude::*,
};
use serenity::model::gateway::GatewayIntents;
use serenity::model::user::User;
use std::sync::Arc;
use log::{debug, error};
use std::fs::File;
use std::io::Read;
use tokio::task;
use serenity::model::prelude::UserId;
use serenity::model::prelude::ChannelId;
use serenity::model::prelude::Ready;
use serenity::async_trait;
use serde_json::json;
use serenity::builder::CreateInvite;
use serenity::json as sjson;

pub struct Clients {
    pub main: Arc<serenity::CacheAndHttp>,
    pub servers: Arc<serenity::CacheAndHttp>,
    pub fetcher: serenity::http::Http,
}

pub struct Database {
    pub redis: deadpool_redis::Pool,
    pub clis: Clients,
    tokens: BaypawTokens,
}

#[derive(Serialize, Deserialize, Clone)]
struct BaypawTokens {
    token_main: String,
    token_server: String,
    token_squirrelflight: String,
    token_fetch_bot_1: String,
}

struct MainHandler;

#[async_trait]
impl EventHandler for MainHandler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        debug!("{} is connected!", ready.user.name);
    }
}

impl Database {
    pub async fn new() -> Self {
        let cfg = Config::from_url("redis://localhost:1001/1");
        let path = match env::var_os("HOME") {
            None => { panic!("$HOME not set"); }
            Some(path) => PathBuf::from(path),
        };

        let data_dir = path.into_os_string().into_string().unwrap() + "/FatesList/config/data/";        
        debug!("Got data dir: {}", data_dir);
        let mut file = File::open(data_dir.to_owned() + "secrets.json").expect("No config file found");
        let mut discord = String::new();
        file.read_to_string(&mut discord).unwrap();

        let tokens: BaypawTokens = serde_json::from_str(&discord).expect("secrets.json was not well-formatted");

        // Login main, server and squirrelflight using serenity
        
        // Main client
        let mut main_cli = Client::builder(&tokens.token_main.clone())
        .event_handler(MainHandler)
        .intents(GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES | GatewayIntents::GUILD_MEMBERS | GatewayIntents::GUILD_PRESENCES)
        .await
        .unwrap();  
        
        let main_cache = main_cli.cache_and_http.clone();
        
        task::spawn(async move {main_cli.start().await });

        // Server client
        let mut server_cli = Client::builder(&tokens.token_server.clone())
        .intents(GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES | GatewayIntents::GUILD_MEMBERS)
        .await
        .unwrap();

        // Fetch bot 1
        let fetch_bot_1_cli = serenity::http::Http::new_with_token(&tokens.token_fetch_bot_1.clone());

        let server_cache = server_cli.cache_and_http.clone();

        task::spawn(async move {
            let res = server_cli.start().await; 
            if res.is_err() {
                error!("{}", res.err().unwrap());
            }
        });

        Database {
            redis: cfg.create_pool(Some(Runtime::Tokio1)).unwrap(),
            tokens,
            clis: Clients {
                main: main_cache,
                servers: server_cache,
                fetcher: fetch_bot_1_cli
            },
        }
    }

    pub async fn getch(&self, id: u64) -> Option<User> {
        // First check the main_cli
        debug!(
            "Have {count} cached users in main cli",
            count = self.clis.main.cache.user_count(),
        );

        let cached_data = UserId(id).to_user_cached(&self.clis.main.cache).await;

        if cached_data.is_some() {
            return Some(cached_data.unwrap());
        }

        // Then check the server_cli
        debug!(
            "Have {count} cached users in server cli",
            count = self.clis.servers.cache.user_count(),
        );

        let cached_data = UserId(id).to_user_cached(&self.clis.servers.cache).await;

        if cached_data.is_some() {
            return Some(cached_data.unwrap());
        }

        // All failed, lets move to fetch_bot_1
        let fetched = self.clis.fetcher.get_user(id).await;

        if fetched.is_err() {
            error!("{:?}", fetched.unwrap_err());
            return None;
        }
        return Some(fetched.unwrap());
    }

    pub async fn guild_invite(&self, cid: u64, uid: u64) -> Option<String> {
        let b = CreateInvite::default()
            .max_age(60*15)
            .max_uses(1)
            .temporary(false)
            .unique(true)
            .clone();
        
        let map = sjson::hashmap_to_json_map(b.0);

        let chan = self.clis.servers.http.create_invite(cid, 
            &map, 
            Some(
                &format!("Invite created for user {user}",
                user = uid,
            ))).await;
        
        chan.ok().map(|c| c.url())

    }
}