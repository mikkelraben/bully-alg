#[macro_use]
extern crate rocket;

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use k8s_openapi::{api::core::v1::Pod, serde_json::json};
use kube::{
    Client, Error,
    api::{Api, Patch, PatchParams},
};
use rand::seq::IndexedRandom;
use reqwest::{self};
use rocket::{
    fs::NamedFile,
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
    pod_name: Mutex<String>,
    namespace: Mutex<String>,
    doing_election: Mutex<bool>,
}

static STATE: AppState = AppState {
    local_pod: Mutex::new(PodInfo {
        id: Uuid::nil(),
        ip_address: std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
    }),
    coordinator: Mutex::new(None),
    known_pods: Mutex::new(Vec::new()),
    pod_name: Mutex::new(String::new()),
    namespace: Mutex::new(String::new()),
    doing_election: Mutex::new(false),
};

// sends a health check request to the coordinator pod
async fn coordinator_health_check(coordinator: &PodInfo) -> bool {
    let url = format!("http://{}:8080/get-id", coordinator.ip_address);
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(1))
        .send()
        .await;
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
    if *STATE.doing_election.lock().unwrap() {
        return;
    }
    *STATE.doing_election.lock().unwrap() = true;
    println!("Holding election...");
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
                    Ok(text) => {
                        println!("{}", text);
                        text == "true"
                    }
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
        become_coordinator(known_pods).await;
    }
    println!("Election finished.");
    *STATE.doing_election.lock().unwrap() = false;
}

async fn become_coordinator(known_pods: Vec<PodInfo>) {
    // send coordinator messages to all known pods
    for pod in &known_pods {
        let url = format!(
            "http://{}:8080/receive_coordinator/{}",
            pod.ip_address.to_string(),
            STATE.local_pod.lock().unwrap().id
        );
        let _ = reqwest::get(&url).await;
    }

    if let Err(err) = update_coordinator_label(true).await {
        eprintln!("Failed to mark pod as coordinator: {err}");
    }

    // set self as coordinator
    let mut coordinator = STATE.coordinator.lock().unwrap();
    *coordinator = Some(STATE.local_pod.lock().unwrap().clone());
}

async fn find_pods() -> Vec<PodInfo> {
    // discover other pods in the cluster
    let ips = lookup_host("get-pods-service:8080").await.unwrap();

    let mut pods = Vec::new();
    let client = reqwest::Client::new();

    for ip in ips {
        let response = client
            .get(format!("http://{}:8080/get-id", ip.ip()))
            .timeout(std::time::Duration::from_secs(1))
            .send()
            .await;
        //println!("response: {:?}", response);

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

#[get("/<file..>")]
async fn files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("website/").join(file)).await.ok()
}

#[get("/")]
async fn index() -> Option<NamedFile> {
    NamedFile::open(Path::new("website/test_kubernetes.html"))
        .await
        .ok()
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
    println!("Received election message from: {}", other_node_id);
    let other_node_id = other_node_id.parse::<Uuid>().unwrap();
    if other_node_id.as_u128() < STATE.local_pod.lock().unwrap().id.as_u128() {
        rocket::tokio::spawn(hold_election());
        println!("responding true to election message.");
        "true"
    } else {
        println!("responding false to election message.");
        "false"
    }
}

// Endpoint to receive coordinator messages
// This endpoint sets this node's coordinator to the received coordinator
#[get("/receive_coordinator/<coordinator>")]
async fn receive_coordinator(coordinator: String) {
    println!("Received coordinator message: {}", coordinator);
    let coordinator_id = coordinator.parse::<Uuid>().unwrap();
    // find the pod with the given id in known_pods
    if let Some(pod) = STATE
        .known_pods
        .lock()
        .unwrap()
        .iter()
        .find(|pod| pod.id == coordinator_id)
    {
        let pod = pod.clone();
        *STATE.coordinator.lock().unwrap() = Some(pod);
    }
    if let Err(err) = update_coordinator_label(false).await {
        eprintln!("Failed to remove coordinator label: {err}");
    }
}

async fn update_coordinator_label(is_coordinator: bool) -> Result<(), Error> {
    println!("{:?}, {:?}", STATE, is_coordinator);
    let client = Client::try_default().await?;
    let namespace = STATE.namespace.lock().unwrap().clone();
    let pod_name = STATE.pod_name.lock().unwrap().clone();
    let pods: Api<Pod> = Api::namespaced(client, &namespace);

    let patch_body = json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": pod_name.clone(),
            "labels": {
                "coordinator": is_coordinator.to_string()
            }
        }
    });

    let patch = Patch::Apply(patch_body);
    let patch_params = PatchParams::apply("pod-labeler");
    pods.patch(&pod_name, &patch_params, &patch).await?;

    Ok(())
}

#[get("/state")]
fn get_state() -> Json<AppState> {
    let state_clone = AppState {
        local_pod: Mutex::new(STATE.local_pod.lock().unwrap().clone()),
        coordinator: Mutex::new(STATE.coordinator.lock().unwrap().clone()),
        known_pods: Mutex::new(STATE.known_pods.lock().unwrap().clone()),
        pod_name: Mutex::new(STATE.pod_name.lock().unwrap().clone()),
        namespace: Mutex::new(STATE.namespace.lock().unwrap().clone()),
        doing_election: Mutex::new(*STATE.doing_election.lock().unwrap()),
    };
    Json(state_clone)
}

#[get("/fortune")]
fn get_fortune() -> String {
    let fortunes = vec![
        "Today it's up to you to create the peacefulness you long for.",
        "A friend asks only for your time not your money.",
        "If you refuse to accept anything but the best, you very often get it.",
        "A smile is your passport into the hearts of others.",
        "A good way to keep healthy is to eat more Chinese food.",
        "Your high-minded principles spell success.",
        "Hard work pays off in the future, laziness pays off now.",
        "Change can hurt, but it leads a path to something better.",
        "Enjoy the good luck a companion brings you.",
        "People are naturally attracted to you.",
        "Hidden in a valley beside an open stream- This will be the type of place where you will find your dream.",
        "A chance meeting opens new doors to success and friendship.",
        "You learn from your mistakes... You will learn a lot today.",
        "If you have something good in your life, don't let it go!",
        "What ever you're goal is in life, embrace it visualize it, and for it will be yours.",
        "Your shoes will make you happy today.",
        "You cannot love life until you live the life you love.",
        "Be on the lookout for coming events; They cast their shadows beforehand.",
        "Land is always on the mind of a flying bird.",
        "The man or woman you desire feels the same about you.",
        "Meeting adversity well is the source of your strength.",
        "A dream you have will come true.",
        "Our deeds determine us, as much as we determine our deeds.",
        "Never give up. You're not a failure if you don't give up.",
        "You will become great if you believe in yourself.",
        "There is no greater pleasure than seeing your loved ones prosper.",
        "You will marry your lover.",
        "A very attractive person has a message for you.",
        "You already know the answer to the questions lingering inside your head.",
        "It is now, and in this world, that we must live.",
        "You must try, or hate yourself for not trying.",
        "You can make your own happiness.",
    ];
    fortunes.choose(&mut rand::rng()).unwrap().to_string()
}

fn periodic_check() {
    rocket::tokio::spawn(async {
        // initial delay before starting the periodic check
        rocket::tokio::time::sleep(rocket::tokio::time::Duration::from_secs(5)).await;
        let mut interval =
            rocket::tokio::time::interval(rocket::tokio::time::Duration::from_secs(1));
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
    println!("{:?}", local_pod);
    *STATE.local_pod.lock().unwrap() = local_pod;
    *STATE.pod_name.lock().unwrap() = std::env::var("POD_NAME").expect("Expects a pod name");
    *STATE.namespace.lock().unwrap() =
        std::env::var("POD_NAMESPACE").expect("Expects a pod namespace");
    periodic_check();
    rocket::build().mount(
        "/",
        routes![
            index,
            files,
            get_id,
            receive_election,
            receive_coordinator,
            get_fortune,
            get_state
        ],
    )
}
