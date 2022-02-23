use deadpool_redis::{Config, Runtime};
use std::path::PathBuf;
use std::env;
use serde::{Serialize, Deserialize};
use serenity::{
    prelude::*,
};
use serenity::client::bridge::gateway::GatewayIntents;
use serenity::model::user::User;
use std::sync::Arc;
use log::{debug, error};
use std::fs::File;
use std::io::Read;
use serenity::framework::standard::{
    StandardFramework
};
use tokio::task;
use serenity::model::prelude::UserId;

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
        
        let framework = StandardFramework::new()
        .configure(|c| c.prefix("%")); // set the bot's prefix to "~"

        let server_framework = StandardFramework::new()
        .configure(|c| c.prefix("+")); // set the bot's prefix to "~"

        // Main client
        let mut main_cli = Client::builder(&tokens.token_main.clone())
        .framework(framework)
        .intents(GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES | GatewayIntents::GUILD_MEMBERS | GatewayIntents::GUILD_PRESENCES)
        .await
        .unwrap();  
        
        let main_cache = main_cli.cache_and_http.clone();
        
        task::spawn(async move {main_cli.start().await });

        // Server client
        let mut server_cli = Client::builder(&tokens.token_server.clone())
        .framework(server_framework)
        .intents(GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES | GatewayIntents::GUILD_MEMBERS | GatewayIntents::GUILD_PRESENCES)
        .await
        .unwrap();

        // Fetch bot 1
        let fetch_bot_1_cli = serenity::http::Http::new_with_token(&tokens.token_fetch_bot_1.clone());

        let server_cache = server_cli.cache_and_http.clone();

        task::spawn(async move {server_cli.start().await });

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
            count = self.clis.main.cache.user_count().await,
        );

        let cached_data = UserId(id).to_user_cached(&self.clis.main.cache).await;

        if cached_data.is_some() {
            return Some(cached_data.unwrap());
        }

        // Then check the server_cli
        debug!(
            "Have {count} cached users in server cli",
            count = self.clis.servers.cache.user_count().await,
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
}