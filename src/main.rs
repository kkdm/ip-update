use env_logger;
use log::{error, info, debug};
use std::time::Duration;
use snmp::SyncSession;
use structopt::StructOpt;
use std::{process, env};
use serde::{Serialize, Deserialize};
use serde_json;

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(short = "d", long ="destination", default_value = "192.168.0.1")] 
    destination: String,

    #[structopt(short = "D", long ="domain", default_value = "example.com")] 
    domain: String,

    #[structopt(short = "o", long ="stdout")] 
    stdout: bool,

    #[structopt(short = "f", long ="force")] 
    force: bool,

    #[structopt(short = "e", long ="endpoint", default_value = "http://localhost/publish", required_if("stdout", "false"))] 
    endpoint: String,

    #[structopt(short = "S", long ="subject", default_value = "new_ip")] 
    subject: String, 
}

#[derive(Deserialize)]
struct ZoneIdItem {
    id: String,
}

#[derive(Deserialize)]
struct ZoneIdResult {
    result: Vec<ZoneIdItem>,
}

#[derive(Deserialize)]
struct DnsItem {
    content: String,
}

#[derive(Deserialize)]
struct DnsResult {
    result: Vec<DnsItem>,
}

#[derive(Serialize)]
struct PubRequest {
    subject: String,
    message: String,
}

#[derive(Deserialize)]
struct PubResult {
    ok: bool,
    message: String,
}

fn get_possible_ips(dest: &String) -> Option<Vec<(String, String)>> {
    let if_idx_oid = &[1,3,6,1,2,1,4,20,1,2,];
    let community = "private".as_bytes();
    let timeout = Duration::from_secs(5);
    let non_repeaters = 0;
    let max_repetitions = 4;
    
    let mut sess = match SyncSession::new(dest, community, Some(timeout), 0) {
        Ok(sess) => sess,
        Err(_) => return None,
    };

    let response = match sess.getbulk(&[if_idx_oid], non_repeaters, max_repetitions) {
        Ok(res) => res,
        Err(_) => return None,
    };

    let possible_ips: Vec<(String, String)> = response.varbinds
        .filter(|(name, _)| 
            !(name.to_string().contains("127.0.0.1") || 
            name.to_string().contains("192.168.1.1")))
        .map(|(name, val)| 
            (name.to_string().replace("1.3.6.1.2.1.4.20.1.2.", ""),
            format!("{:?}", val).replace("INTEGER: ", "")))
        .collect();
    
    if possible_ips.len() == 0 {
        return None   
    };

    Some(possible_ips)
}

fn get_possible_indexes(dest: &String) -> Option<Vec<String>> {
    let if_desc_oid = &[1,3,6,1,2,1,2,2,1,2,];
    let community = "private".as_bytes();
    let timeout = Duration::from_secs(2);
    let non_repeaters = 0;
    let max_repetitions = 25;

    let mut sess = match SyncSession::new(dest, community, Some(timeout), 0) {
        Ok(sess) => sess,
        Err(_) => return None,
    };

    let response = match sess.getbulk(&[if_desc_oid], non_repeaters, max_repetitions) {
        Ok(res) => res,
        Err(_) => return None,
    };

    let possible_indexes: Vec<String> = response.varbinds
        .filter(|(_, val)| format!("{:?}", val).contains("pppoe-wan1_poe"))
        .map(|(name, _)| name.to_string().replace("1.3.6.1.2.1.2.2.1.2.", ""))
        .collect();

    if possible_indexes.len() == 0 {
        return None
    }

    Some(possible_indexes)
}

fn get_wan_ip(dest: &String) -> Option<String> {

    let possible_ips = match get_possible_ips(dest) {
        Some(ips) => ips,
        None => return None
    };

    let possible_indexes = match get_possible_indexes(dest) {
        Some(indexes) => indexes,
        None => return None
    };

    let mut ip: Vec<String> = possible_ips.into_iter()
        .filter(|(_, idx)| possible_indexes.contains(idx))
        .map(|(ip, _)| ip).collect();
    
    if ip.len() == 0 {
        return None
    };

    ip.pop()
}

fn get_zone_id(token: &String, domain: &String) -> Option<String> {
    let resp = 
        ureq::get("https://api.cloudflare.com/client/v4/zones")
              .query("name", domain)
              .set("Authorization", format!("Bearer {}", token).as_str())
              .set("Content-Type", "application/json")
              .call();

    match resp.into_json_deserialize::<ZoneIdResult>() {
        Ok(ref mut obj) => {
            if let Some(ZoneIdItem { id }) = obj.result.pop() {
                Some(id.to_string())
            }
            else {
                None
            }
        },
        Err(_) => None
    }
}

fn get_ip(token: &String, zone_id: &String) -> Option<String> {
    let endpoint = 
        format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records", zone_id);

    let resp = 
        ureq::get(&endpoint.as_str())
              .query("type", "A")
              .set("Authorization", format!("Bearer {}", token).as_str())
              .set("Content-type", "application/json")
              .call();

    match resp.into_json_deserialize::<DnsResult>() {
        Ok(ref mut obj) => {
            if let Some(DnsItem { content }) = obj.result.pop() {
                Some(content.to_string())
            }
            else {
                None
            }
        },
        Err(_) => None,
    }
}

fn get_current_ip(token: &String, doman: &String) -> Result<String, String> {
    let zone_id = match get_zone_id(token, doman) {
        Some(id) => id,
        None => return Err("couldn't get zone id".to_string()),
    };

    match get_ip(token, &zone_id) {
        Some(ip) => Ok(ip),
        None => return Err("couldn't get current ip address".to_string()),
    }
}

fn publish_new_ip(new_ip: &String, endpoint: &String, subject: &String, domain: &String) -> Result<(), String> {
    let message = PubRequest {
        subject: subject.clone(),
        message: format!("{}:{}", domain, new_ip),
    };
    let resp = 
        ureq::post(&format!("{}", endpoint).as_str())
              .set("Content-type", "application/json")
              .send_json(serde_json::value::to_value(message).unwrap());

    if !resp.ok() {
        return Err(resp.into_string().unwrap_or("could not get response".to_string()));
    };

    match resp.into_json_deserialize::<PubResult>() {
        Ok(ref mut obj) if !obj.ok => {
            return Err(format!("failed to publish message: {}", obj.message));
        },
        Ok(ref mut _obj) => {
            Ok(())
        },
        Err(_) => Err("failed to parse response".to_string())
    }    
}

fn main() {
    env::set_var("RUST_LOG", "info");
    env_logger::init();
    let opt = Opt::from_args();

    let token = match env::var("TOKEN") {
        Ok(token) => token,
        Err(_) => {
            error!("environment variable TOKEN not defined");
            process::exit(1);
        }
    };

    let wan_ip = match get_wan_ip(&opt.destination) {
        Some(ip) => ip,
        None => {
            error!("could not get ip from device");
            process::exit(1);
        }
    };

    let current_ip = match get_current_ip(&token, &opt.domain) {
        Ok(ip) => ip,
        Err(e) => {
            error!("error: {}", e);
            process::exit(1);
        }
    };

    if wan_ip == current_ip && !opt.force {
        debug!("no ip change");
        process::exit(0);        
    }

    if opt.stdout {
        println!("{}", wan_ip);
        process::exit(0);        
    };

    if let Err(e) = publish_new_ip(&wan_ip, &opt.endpoint, &opt.subject, &opt.domain) {
        error!("failed to publish new ip: {}", e);
    };

    info!("published new ip: {}, server: {}", &wan_ip, &opt.endpoint);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get_current_ip() {
        let token = env::var("TOKEN").unwrap();
        assert_eq!(get_current_ip(&token, &"kdmw.dev".to_string()).unwrap(), "60.236.13.148");
    }
}
