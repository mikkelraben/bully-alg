#[macro_use]
extern crate rocket;

use std::sync::Mutex;

use reqwest::{self};
use rocket::{
    serde::{Deserialize, Serialize, json::Json},
    tokio::net::lookup_host,
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PodInfo {
    id: uuid::Uuid,
    ip_address: std::net::IpAddr,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppState {
    local_pod: Mutex<PodInfo>,
    coordinator: Mutex<Option<PodInfo>>,
    known_pods: Mutex<Vec<PodInfo>>,
}

static STATE: AppState = AppState {
    local_pod: Mutex::new(PodInfo {
        id: Uuid::nil(),
        ip_address: std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
    }),
    coordinator: Mutex::new(None),
    known_pods: Mutex::new(Vec::new()),
};

// sends a health check request to the coordinator pod
async fn coordinator_health_check(coordinator: &PodInfo) -> bool {
    let url = format!("http://{}:8080/get-id", coordinator.ip_address);
    let response = reqwest::get(&url).await;
    // get the id from the coordinator pod
    match response {
        // if we get a response, check if the id matches the coordinator's id
        Ok(resp) => match resp.text().await {
            Ok(text) => text == coordinator.id.to_string(),
            Err(_) => return false,
        },
        Err(_) => return false,
    }
}

async fn hold_election() {
    // send election messages to all known pods with higher IDs
    // if no one responds, become the coordinator
    // if someone responds, wait for a coordinator message
    let local_pod_id = STATE.local_pod.lock().unwrap().id;
    let known_pods = STATE.known_pods.lock().unwrap().clone();
    let higher_id_pods: Vec<&PodInfo> = known_pods
        .iter()
        .filter(|pod| pod.id > local_pod_id)
        .collect();

    // for each node with a higher id, send an election message and wait for a response
    let is_coordinator =
        !futures::future::join_all(higher_id_pods.iter().map(async |pod: &&PodInfo| {
            // Where to send the election message
            let url = format!(
                "http://{}:8080/receive_election/{}",
                pod.ip_address.to_string(),
                local_pod_id
            );
            // Actually send the election message
            let response = reqwest::get(&url).await;

            // parse the response from the pod
            match response {
                Ok(resp) => match resp.text().await {
                    Ok(text) => text == "true",
                    Err(_) => false,
                },
                Err(_) => false,
            }
        }))
        // if any pod responded true, then we are not the coordinator
        .await
        .iter()
        .any(|&response| response);

    if is_coordinator {
        // send coordinator messages to all known pods
        for pod in &known_pods {
            let url = format!(
                "http://{}:8080/receive_coordinator/{}",
                pod.ip_address.to_string(),
                STATE.local_pod.lock().unwrap().id
            );
            let _ = reqwest::get(&url).await;
        }
        // set self as coordinator
        let mut coordinator = STATE.coordinator.lock().unwrap();
        *coordinator = Some(STATE.local_pod.lock().unwrap().clone());
    }
}

async fn find_pods() -> Vec<PodInfo> {
    // discover other pods in the cluster
    let ips = lookup_host("get-pods-service:8080").await.unwrap();

    let mut pods = Vec::new();

    for ip in ips {
        let response = reqwest::get(format!("http://{}:8080/get-id", ip.ip())).await;
        println!("response: {:?}", response);

        let id = match response {
            Ok(resp) => match resp.text().await {
                Ok(text) => text.parse::<Uuid>().ok(),
                Err(_) => None,
            },
            Err(_) => None,
        };

        println!("Discovered pod with IP: {:?}, ID: {:?}", ip.ip(), id);

        if let Some(id) = id {
            pods.push(PodInfo {
                id,
                ip_address: ip.ip(),
            });
        }
    }
    pods.iter()
        .filter(|pod| pod.id != STATE.local_pod.lock().unwrap().id)
        .cloned()
        .collect()
}

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

// Endpoint to get the ID of the local pod
#[get("/get-id")]
fn get_id() -> String {
    STATE.local_pod.lock().unwrap().id.to_string()
}

// Endpoint to receive election messages
// This endpoint returns whether or not this node has a higher ID than the election message
// if it does, it begins its own election
#[get("/receive_election/<other_node_id>")]
async fn receive_election(other_node_id: String) -> &'static str {
    let other_node_id = other_node_id.parse::<Uuid>().unwrap();
    if other_node_id.as_u128() > STATE.local_pod.lock().unwrap().id.as_u128() {
        rocket::tokio::spawn(hold_election());
        "true"
    } else {
        "false"
    }
}

// Endpoint to receive coordinator messages
// This endpoint sets this node's coordinator to the received coordinator
#[get("/receive_coordinator/<coordinator>")]
fn receive_coordinator(coordinator: String) {
    let coordinator_id = coordinator.parse::<Uuid>().unwrap();
    // find the pod with the given id in known_pods
    if let Some(pod) = STATE
        .known_pods
        .lock()
        .unwrap()
        .iter()
        .find(|pod| pod.id == coordinator_id)
    {
        *STATE.coordinator.lock().unwrap() = Some(pod.clone());
    }
}

#[get("/state")]
fn get_state() -> Json<AppState> {
    let state_clone = AppState {
        local_pod: Mutex::new(STATE.local_pod.lock().unwrap().clone()),
        coordinator: Mutex::new(STATE.coordinator.lock().unwrap().clone()),
        known_pods: Mutex::new(STATE.known_pods.lock().unwrap().clone()),
    };
    Json(state_clone)
}

fn periodic_check() {
    rocket::tokio::spawn(async {
        // initial delay before starting the periodic check
        rocket::tokio::time::sleep(rocket::tokio::time::Duration::from_secs(5)).await;
        let mut interval =
            rocket::tokio::time::interval(rocket::tokio::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            *STATE.known_pods.lock().unwrap() = find_pods().await;
            let coordinator_opt = STATE.coordinator.lock().unwrap().clone();
            if let Some(coordinator) = coordinator_opt {
                if !coordinator_health_check(&coordinator).await {
                    hold_election().await;
                }
            } else {
                hold_election().await;
            }
        }
    });
}

#[launch]
async fn rocket() -> _ {
    let ip_str = std::env::var("POD_IP").unwrap_or("0.0.0.0".into());
    let ip_address: std::net::Ipv4Addr = ip_str.parse().expect("Invalid POD_IP format");
    let local_pod = PodInfo {
        id: Uuid::new_v4(),
        ip_address: std::net::IpAddr::V4(ip_address),
    };
    *STATE.local_pod.lock().unwrap() = local_pod;
    periodic_check();
    rocket::build().mount(
        "/",
        routes![
            index,
            get_id,
            receive_election,
            receive_coordinator,
            get_state
        ],
    )
}
