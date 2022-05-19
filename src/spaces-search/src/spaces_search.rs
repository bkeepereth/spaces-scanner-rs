use conf::{parse_args, get_config, init_logger};

use log::info;
use std::result::Result;
use std::collections::BTreeMap;
use std::collections::HashSet;
use clap::ArgMatches;

use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;

use std::thread;
use std::time;
use reqwest::StatusCode;

use chrono::{Local, DateTime, Utc};

fn usage() {
    println!("Usage: cargo run -- --input <input> --mode <file/text> --config <config> --rate <rate>");
}

fn read_lines<P>(file: P) -> io::Result<io::Lines<io::BufReader<File>>> where P: AsRef<Path>, {
    let f = File::open(file)?;
    Ok(io::BufReader::new(f).lines())
}

fn format_elapsed(started_at: &str) -> Result<String, Box<dyn std::error::Error>> {
    let start_ts = DateTime::parse_from_rfc3339(started_at)?;
    let elapsed = DateTime::<Utc>::from_utc(Local::now().naive_utc(), Utc)
        .signed_duration_since(start_ts);

    let elapsed_hr = (elapsed.num_seconds() / 60) / 60;
    let elapsed_min = (elapsed.num_seconds() / 60) - (elapsed_hr * 60);
    let elapsed_seconds = elapsed.num_seconds() - (elapsed_min * 60) - (elapsed_hr * 3600);

    let min_label = match elapsed_min {
        0..=9 => format!("0{}", elapsed_min),
        10..=59 => format!("{}", elapsed_min),
        _ => String::from("00"),
    };

    let sec_label = match elapsed_seconds {
        0..=9 => format!("0{}", elapsed_seconds),
        10..=59 => format!("{}", elapsed_seconds),
        _ => String::from("00"),
    };

    Ok(format!("{}:{}:{}", elapsed_hr, min_label, sec_label))
}

async fn space_search(bearer_token: &str, topic: &str, space_set: &mut HashSet<String>) -> Result<(), Box<dyn std::error::Error>> {
    info!("space_search|starting");

    let search_url = String::from("https://api.twitter.com/2/spaces/search");

    let tw_client = reqwest::Client::new();
    let mut response = tw_client.get(&search_url)
        .query(&[
            ("query", topic),
            ("space.fields", &"title,started_at,participant_count"),
            ("expansions", &"creator_id,host_ids,speaker_ids"),
        ])
        .header("Authorization", format!("Bearer {}", bearer_token))
        .send()?;

    match response.status() {
        StatusCode::OK => info!("space_search|query success"),
        s => info!("space_search|status: {}", s),
    }

    let tmp: serde_json::Value = match response.text() {
        Ok(x) => { serde_json::from_str(&x)? },
        Err(_) => { return Ok(()) },  // abort if unable to parse
    };

    let includes = match tmp["includes"].as_object() {
        Some(x) => { x },
        None => { return Ok(()) },
    };
                    
    let data = match tmp["data"].as_array() {
        Some(x) => { x },
        None => { return Ok(()) },
    };

    for space in data {
        let mut space_id = space["id"].to_string(); 
        let mut state = space["state"].to_string(); 

        space_id = space_id[1..space_id.len()-1].to_string();
        state = state[1..state.len()-1].to_string();

        if (state != "live") || (space_set.contains(&space_id)) { continue; }

        //let mut creator_id = space["creator_id"].to_string();
        let participant_count = space["participant_count"].to_string();
        let mut started_at = space["started_at"].to_string();
        let mut title = space["title"].to_string();

        //creator_id = creator_id[1..creator_id.len()-1].to_string();
        started_at = started_at[1..started_at.len()-1].to_string();
        title = title[1..title.len()-1].to_string();

        let speaker_ids = match space["speaker_ids"].as_array() {
            Some(ids) => { ids },
            None => { continue },        // skip on bad input
        };

        let host_ids = match space["host_ids"].as_array() {
            Some(ids) => { ids },
            None => { continue },
        };

        let users = match includes["users"].as_array() {
            Some(users) => { users },
            None => { continue },
        };

        print!("--------------------------------------------------------------------------+++++\n{}\n{}\n\n",
            title,
            started_at
        );

        let mut count = 0;
        let mut cohosts: HashSet<String> = HashSet::new();
        for host in host_ids {
            for user in users {
                if host == &user["id"] {
                    let mut name = user["name"].to_string();
                    let mut username = user["username"].to_string();

                    name = name[1..name.len()-1].to_string();
                    username = username[1..username.len()-1].to_string();
                               
                    if count == 0 {
                        print!("    Host: {} // {}\n", name, username);
                        count += 1;
                    } else {
                        print!("      Co-Host: {} // {}\n", name, username);
                        cohosts.insert(host.to_string());
                        count += 1;
                    }
                }
            }
        }

        for speaker in speaker_ids {
            for user in users {
                if speaker == &user["id"] && !cohosts.contains(&speaker.to_string()) {
                    let mut name = user["name"].to_string();
                    let mut username = user["username"].to_string();

                    name = name[1..name.len()-1].to_string();
                    username = username[1..username.len()-1].to_string();
                                     
                    print!("        Speaker: {} // {}\n", name, username);

                    count += 1;
                }
            }
        }
        print!("\n=> https://twitter.com/i/spaces/{}/peek\n", space_id);
        
        /*
        let start_ts = DateTime::parse_from_rfc3339(&started_at)?;
        let elapsed = DateTime::<Utc>::from_utc(Local::now().naive_utc(), Utc)
            .signed_duration_since(start_ts);

        let elapsed_hr = (elapsed.num_seconds() / 60) / 60;
        let elapsed_min = (elapsed.num_seconds() / 60) - (elapsed_hr * 60);
        let elapsed_seconds = elapsed.num_seconds() - (elapsed_min * 60) - (elapsed_hr * 3600);

        let min_label = match elapsed_min {
            0..=9 => format!("0{}", elapsed_min),
            10..=59 => format!("{}", elapsed_min),
            _ => String::from("00"),
        };

        let sec_label = match elapsed_seconds {
            0..=9 => format!("0{}", elapsed_seconds),
            10..=59 => format!("{}", elapsed_seconds),
            _ => String::from("00"),
        };*/
        let format_elapsed = format_elapsed(&started_at).unwrap();

        println!("=> Started: -{}", format_elapsed);          
        print!("=> Guests: {}\n", participant_count);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args: ArgMatches = parse_args();

    let config_name = cli_args.value_of("conf").expect("ERR: cli [configuration] is invalid");
    let config: BTreeMap<String, String> = get_config(config_name);

    let dt = Utc::now().to_rfc3339();
    let log_dir = String::from(config.get("log_dir").expect("ERR: log_dir is invalid"));
    let log_path = format!("{}/{}_ct_nlp_cli.log", &log_dir, &dt[0..19]);

    init_logger(&log_path);
    info!("main|starting");

    let rate = String::from(cli_args.value_of("rate").expect("ERR: cli [rate] is invalid"));
    let rate_millis = rate.parse::<u64>().unwrap() * 1000;
    match rate_millis {
        0 => panic!("ERR: use recommended rates: 180 - 500"),
        180.. => info!("rate (ms): {}", rate_millis),
        _ => info!("experimental mode // rate (ms): {}", rate_millis),
    };

    let mode = String::from(cli_args.value_of("mode").expect("ERR: cli [mode] is invalid"));
    match mode.as_str() {
        "file" => {
            let topics_path = format!(
                "{}/{}", 
                config.get("etc_dir").expect("ERR: config [etc_dir] is invalid"),
                config.get("topics_file").expect("ERR: config [topics_file] is invalid")
            );

            loop {
                let mut space_set: HashSet<String> = HashSet::new();
                let lines = match read_lines(topics_path.clone()) {
                    Ok(x) => x,
                    Err(err) => panic!("error: {}|topics_path: {}", err, topics_path),
                };

                for line in lines {
                    let topic = match line {
                        Ok(t) => { t },
                        Err(_) => { continue },  // skip on bad input
                    };
                    
                    match space_search(
                        config.get("bearer_token").unwrap(),
                        &topic,
                        &mut space_set
                    ).await {
                        Ok(result) => { info!("{:?}", result); },
                        Err(_) => { continue; },
                    };
                }

                thread::sleep(time::Duration::from_millis(rate_millis));
            }
        },
        "text" => {
            loop {
                let mut space_set: HashSet<String> = HashSet::new();
                let topic = cli_args.value_of("input").expect("ERR: input [cli] is invalid");

                match space_search(
                    config.get("bearer_token").unwrap(),
                    &topic,
                    &mut space_set
                ).await {
                    Ok(result) => { info!("{:?}", result); },
                    Err(err) => { info!("{}", err); },
                };

                thread::sleep(time::Duration::from_millis(rate_millis));
            }
        },
        _ => { usage(); }
    }
    Ok(())
}
