use bristlefrost::models::Status;
use deadpool_redis::{Config, Runtime};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serenity::async_trait;
use serenity::builder::CreateInvite;
use serenity::json as sjson;
use serenity::model::gateway::GatewayIntents;
use serenity::model::guild::Member;
use serenity::model::id::{ChannelId, RoleId};
use serenity::model::prelude::{GuildId, Ready, UserId};
use serenity::model::user::{OnlineStatus, User};
use serenity::prelude::*;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task;

pub struct Clients {
    pub main: Arc<serenity::CacheAndHttp>,
    pub servers: Arc<serenity::CacheAndHttp>,
    pub fetcher: serenity::http::Http,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct StaffRole {
    pub id: String,
    pub staff_id: String,
    pub perm: f32,
    pub fname: String,
}

/// This is what is used for premium and other stuff
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NormalRole {
    pub id: u64,
    pub flag: i32,
    pub fname: String,
}

pub struct Database {
    pub redis: deadpool_redis::Pool,
    pub pool: sqlx::PgPool,
    pub clis: Clients,
    pub staff_roles: HashMap<String, StaffRole>,
    pub normal_roles: HashMap<String, NormalRole>,
    pub discord: Discord,
    /// staffRoleCache maps the ID to its key
    pub staff_roles_cache: HashMap<u64, String>,
    /// normalRoleCache maps the ID to its key
    pub normal_roles_cache: HashMap<u64, String>,
}

#[derive(Deserialize, Clone)]
struct BaypawTokens {
    token_main: String,
    token_squirrelflight: String,
    token_fetch_bot_1: String,
}

#[derive(Deserialize, Clone)]
pub struct Discord {
    pub servers: Servers,
    pub channels: Channels,
}

#[derive(Deserialize, Clone)]
pub struct Channels {
    pub site_errors_channel: String,
}

#[derive(Deserialize, Clone)]
pub struct Servers {
    pub main: GuildId,
}

// A ISuer is a internal user struct
pub struct IUser {
    pub user: User,
    pub status: Status,
}

struct MainHandler {
    normal_roles: HashMap<String, NormalRole>,
    pool: sqlx::PgPool,
    discord: Discord,
}

#[async_trait]
impl EventHandler for MainHandler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        debug!("{} is connected!", ready.user.name);
    }

    async fn guild_member_update(&self, ctx: Context, _: Option<Member>, new: Member) {
        let mut added_flags = Vec::new();

        for (name, role) in &self.normal_roles {
            if added_flags.contains(&role.flag) {
                // Aready dealt with this flag
                continue;
            }

            debug!("Checking if user has {}", name);

            if new.roles.contains(&RoleId(role.id)) {
                debug!("User has {}", name);
                // Remove old flags first
                let res = sqlx::query!(
                    "SELECT flags FROM users WHERE user_id = $1",
                    new.user.id.0 as i64
                )
                .fetch_one(&self.pool)
                .await;

                if let Err(err) = res {
                    let site_errors_channel =
                        self.discord.channels.site_errors_channel.parse::<u64>();

                    error!("Error updating user flags: {}", err);

                    if let Ok(site_errors_channel) = site_errors_channel {
                        let channel = ChannelId(site_errors_channel);
                        let _ = channel
                            .say(
                                &ctx,
                                format!("Error occured when updating flags: ```rs\n{:?}```", err),
                            )
                            .await;
                    }
                } else if let Ok(flags) = res {
                    if flags.flags.contains(&role.flag) {
                        debug!("User already has {}", name);
                        continue;
                    }

                    debug!("User does not have {}. Adding now", name);
                    
                    added_flags.push(role.flag);

                    let res = sqlx::query!(
                        "UPDATE users SET flags = array_remove(flags, $1) WHERE user_id = $2",
                        role.flag,
                        new.user.id.0 as i64
                    )
                    .execute(&self.pool)
                    .await;

                    if let Err(err) = res {
                        let site_errors_channel =
                            self.discord.channels.site_errors_channel.parse::<u64>();

                        error!("Error updating user flags: {}", err);

                        if let Ok(site_errors_channel) = site_errors_channel {
                            let channel = ChannelId(site_errors_channel);
                            let err = channel
                                .say(
                                    &ctx,
                                    format!(
                                        "Error occured when updating flags: ```rs\n{:?}```",
                                        err
                                    ),
                                )
                                .await;

                            if let Err(e) = err {
                                error!("Error sending message to Discord, {:?}", e);
                            }
                        }
                    }

                    let res = sqlx::query!(
                        "UPDATE users SET flags = array_append(flags, $1) WHERE user_id = $2",
                        role.flag,
                        new.user.id.0 as i64
                    )
                    .execute(&self.pool)
                    .await;

                    if let Err(err) = res {
                        let site_errors_channel =
                            self.discord.channels.site_errors_channel.parse::<u64>();

                        error!("Error updating user flags: {}", err);

                        if let Ok(site_errors_channel) = site_errors_channel {
                            let channel = ChannelId(site_errors_channel);
                            let err = channel
                                .say(
                                    &ctx,
                                    format!(
                                        "Error occured when updating flags: ```rs\n{:?}```",
                                        err
                                    ),
                                )
                                .await;

                            if let Err(e) = err {
                                error!("Error sending message to Discord, {:?}", e);
                            }
                        }
                    }
                }
            } else {
                debug!("User does not have {}", name);
                let res = sqlx::query!(
                    "UPDATE users SET flags = array_remove(flags, $1) WHERE user_id = $2",
                    role.flag,
                    new.user.id.0 as i64
                )
                .execute(&self.pool)
                .await;

                if let Err(err) = res {
                    let site_errors_channel =
                        self.discord.channels.site_errors_channel.parse::<u64>();

                    error!("Error updating user flags: {}", err);

                    if let Ok(site_errors_channel) = site_errors_channel {
                        let channel = ChannelId(site_errors_channel);
                        let err = channel
                            .say(
                                &ctx,
                                format!("Error occured when updating flags: ```rs\n{:?}```", err),
                            )
                            .await;

                        if let Err(e) = err {
                            error!("Error sending message to Discord, {:?}", e);
                        }
                    }
                }
            }
        }
    }
}

impl Database {
    pub async fn new() -> Self {
        const MAX_CONNECTIONS: u32 = 3; // max connections to the database, we don't need too many here

        let pool = PgPoolOptions::new()
            .max_connections(MAX_CONNECTIONS)
            .connect("postgres://localhost/fateslist")
            .await
            .expect("Could not initialize connection");

        info!("Connected to database");

        let cfg = Config::from_url("redis://localhost:1001/1");
        let path = match env::var_os("HOME") {
            None => {
                panic!("$HOME not set");
            }
            Some(path) => PathBuf::from(path),
        };

        let data_dir = path.into_os_string().into_string().unwrap() + "/FatesList/config/data/";
        debug!("Got data dir: {}", data_dir);

        let mut file =
            File::open(data_dir.to_owned() + "secrets.json").expect("No config file found");
        let mut discord = String::new();
        file.read_to_string(&mut discord).unwrap();

        let tokens: BaypawTokens =
            serde_json::from_str(&discord).expect("secrets.json was not well-formatted");

        let mut staff_file = File::open(data_dir.to_owned() + "staff_roles.json")
            .expect("No staff roles file found");
        let mut staff_str = String::new();
        staff_file.read_to_string(&mut staff_str).unwrap();

        let staff_roles: HashMap<String, StaffRole> =
            serde_json::from_str(&staff_str).expect("staff_roles.json was not well-formatted");

        let mut normal_roles_file =
            File::open(data_dir.to_owned() + "roles.json").expect("No normal roles file found");
        let mut normal_str = String::new();
        normal_roles_file.read_to_string(&mut normal_str).unwrap();

        let normal_roles: HashMap<String, NormalRole> =
            serde_json::from_str(&normal_str).expect("roles.json was not well-formatted");

        let mut staff_roles_cache = HashMap::new();

        let mut normal_roles_cache = HashMap::new();

        // This is needed to create a bi-directional cache allowing the mapping of role ids to keys as well as keys to role ids
        for (key, role) in &staff_roles {
            // This sort of copying is rather cheap
            staff_roles_cache.insert(role.id.parse::<u64>().unwrap(), key.clone());
        }

        // This is needed to create a bi-directional cache allowing the mapping of role ids to keys as well as keys to role ids
        for (key, role) in &normal_roles {
            // This sort of copying is rather cheap
            normal_roles_cache.insert(role.id, key.clone());
        }

        let mut discord_file =
            File::open(data_dir.to_owned() + "discord.json").expect("No discord.json file found");
        let mut discord_str = String::new();
        discord_file.read_to_string(&mut discord_str).unwrap();

        let discord: Discord =
            serde_json::from_str(&discord_str).expect("discord.json was not well-formatted");

        // Login main, server and squirrelflight using serenity

        // Main client
        let mut main_cli = Client::builder(
            &tokens.token_main.clone(),
            GatewayIntents::GUILDS
                | GatewayIntents::GUILD_MESSAGES
                | GatewayIntents::GUILD_MEMBERS
                | GatewayIntents::GUILD_PRESENCES,
        )
        .event_handler(MainHandler {
            normal_roles: normal_roles.clone(),
            pool: pool.clone(),
            discord: discord.clone(),
        })
        .await
        .unwrap();

        let main_cache = main_cli.cache_and_http.clone();

        task::spawn(async move { main_cli.start().await });

        // Server client
        let mut server_cli = Client::builder(
            &tokens.token_squirrelflight.clone(),
            GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES,
        )
        .await
        .unwrap();

        // Fetch bot 1
        let fetch_bot_1_cli = serenity::http::Http::new(&tokens.token_fetch_bot_1.clone());

        let server_cache = server_cli.cache_and_http.clone();

        task::spawn(async move {
            let res = server_cli.start().await;
            if res.is_err() {
                error!("{}", res.err().unwrap());
            }
        });

        Database {
            redis: cfg.create_pool(Some(Runtime::Tokio1)).unwrap(),
            clis: Clients {
                main: main_cache,
                servers: server_cache,
                fetcher: fetch_bot_1_cli,
            },
            staff_roles,
            pool,
            staff_roles_cache,
            normal_roles,
            normal_roles_cache,
            discord,
        }
    }

    pub async fn get_user_perms(&self, id: u64) -> &StaffRole {
        let mut perms = self.staff_roles.get("user").unwrap();

        if let Some(member) = self
            .clis
            .main
            .cache
            .member(self.discord.servers.main, UserId(id))
        {
            /*
             * Iterate over every role the member has and check if its in staff_role_cache or not
             * This is also more optimized than looping over all of staff_roles as we only loop
             * over all roles once
             */
            for role in member.roles {
                if let Some(possible_perm) = self.staff_roles_cache.get(&role.0) {
                    let possible = self.staff_roles.get(possible_perm).unwrap();
                    if possible.perm > perms.perm {
                        perms = possible;
                    }
                }
            }
        }

        perms
    }

    pub async fn get_normal_roles(&self, id: u64) -> &NormalRole {
        let mut flags = self.normal_roles.get("user").unwrap();

        if let Some(member) = self
            .clis
            .main
            .cache
            .member(self.discord.servers.main, UserId(id))
        {
            /*
             * Iterate over every role the member has and check if its in normal_role_cache or not
             * This is also more optimized than looping over all of normal_roles as we only loop
             * over all roles once
             */
            for role in member.roles {
                if let Some(possible_flag) = self.normal_roles_cache.get(&role.0) {
                    let possible = self.normal_roles.get(possible_flag).unwrap();

                    if possible.flag > flags.flag {
                        flags = possible;
                    }
                }
            }
        }

        flags
    }

    pub async fn getch(&self, id: u64) -> Option<IUser> {
        // First check the main_cli
        debug!(
            "Have {count} cached users in main cli",
            count = self.clis.main.cache.user_count(),
        );

        let user_id = UserId(id);

        let cached_data = user_id.to_user_cached(&self.clis.main.cache).await;

        if cached_data.is_some() {
            let cached_data = cached_data.unwrap();

            for id in self.clis.main.cache.guilds() {
                let p_opt = self
                    .clis
                    .main
                    .cache
                    .guild_field(id, |guild| guild.presences.clone());
                if let Some(p) = p_opt {
                    let status = p.get(&user_id);

                    if let Some(status) = status {
                        return Some(IUser {
                            user: cached_data,
                            status: match status.status {
                                OnlineStatus::Online => Status::Online,
                                OnlineStatus::Idle => Status::Idle,
                                OnlineStatus::DoNotDisturb => Status::DoNotDisturb,
                                OnlineStatus::Invisible => Status::Offline,
                                OnlineStatus::Offline => Status::Offline,
                                _ => Status::Unknown,
                            },
                        });
                    }
                }
            }

            return Some(IUser {
                user: cached_data,
                status: Status::Unknown,
            });
        }

        // Then check the server_cli
        debug!(
            "Have {count} cached users in server cli",
            count = self.clis.servers.cache.user_count(),
        );

        let cached_data = user_id.to_user_cached(&self.clis.servers.cache).await;

        // No presence intent so....
        if cached_data.is_some() {
            return Some(IUser {
                user: cached_data.unwrap(),
                status: Status::Unknown,
            });
        }

        // All failed, lets move to fetch_bot_1
        let fetched = self.clis.fetcher.get_user(id).await;

        if fetched.is_err() {
            error!("{:?}", fetched.unwrap_err());
            return None;
        }
        Some(IUser {
            user: fetched.unwrap(),
            status: Status::Unknown,
        })
    }

    pub async fn guild_invite(&self, cid: u64, uid: u64) -> Option<String> {
        let b = CreateInvite::default()
            .max_age(60 * 15)
            .max_uses(1)
            .temporary(false)
            .unique(true)
            .clone();

        let map = sjson::hashmap_to_json_map(b.0);

        let chan = self
            .clis
            .servers
            .http
            .create_invite(
                cid,
                &map,
                Some(&format!("Invite created for user {user}", user = uid,)),
            )
            .await;

        chan.ok().map(|c| c.url())
    }
}
