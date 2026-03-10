#![allow(dead_code)]

mod miner;
mod pow;
mod protocol;
mod state;
mod strategy;

#[allow(unused_imports)]
use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
use std::thread;
#[allow(unused_imports)]
use std::time::Duration;

use tungstenite::{connect, Message};
#[allow(unused_imports)]
use uuid::Uuid;

use protocol::{ClientMsg, ServerMsg};

use crate::miner::MineRequest;

const SERVER_URL: &str = "ws://127.0.0.1:4004/ws";
const TEAM_NAME: &str = "EQUIPE_MRW";
const AGENT_NAME: &str = "AYNS";
const NUM_MINERS: usize = 4;

fn main() {
    println!("[*] Connexion à {SERVER_URL}...");
    let ( ws, _response) = connect(SERVER_URL).expect("impossible de se connecter au serveur");
    let ws = Arc::new(Mutex::new(ws));
    println!("[*] Connecté !");

    let agent_id: Uuid = {
        let mut guard = ws.lock().unwrap();
     match read_server_msg(&mut *guard) {
        Some(ServerMsg::Hello { agent_id, tick_ms }) => {
            println!("[*] Hello reçu : agent_id={agent_id}, tick={tick_ms}ms");
            agent_id
        }
        other => panic!("premier message inattendu : {other:?}"),
    }
};
{
    let mut guard = ws.lock().unwrap();
    send_client_msg(
        &mut *guard,
        &ClientMsg::Register {
            team: TEAM_NAME.into(),
            name: AGENT_NAME.into(),
        },
    );}
    println!("[*] Enregistré en tant que {AGENT_NAME} (équipe {TEAM_NAME})");

    // 1. État partagé
    let shared_state = state::new_shared_state(agent_id);

    // 2. Pool de mineurs
    let miner_pool = miner::MinerPool::new(NUM_MINERS);

    // 3. Stratégie
    let strategy: Box<dyn strategy::Strategy> = Box::new(strategy::NearestResourceStrategy);

    // 4. Channel pour que le thread lecteur envoie les messages au thread principal
    let (tx, rx) = std::sync::mpsc::channel::<ServerMsg>();

    // Thread lecteur WS : lit en boucle, met à jour le state, forward via tx
    let shared_state_reader = Arc::clone(&shared_state);
    let tx_clone = tx.clone();
    let ws_reader = Arc::clone(&ws);
    thread::spawn(move || {
        loop {
            let msg_text = {
                let mut guard = ws_reader.lock().unwrap();
                guard.read().unwrap().into_text().unwrap()
            };
            let parsed: ServerMsg = serde_json::from_str(&msg_text).unwrap();
            shared_state_reader.lock().unwrap().update(&parsed);
            tx_clone.send(parsed).unwrap();

        }
    });

    // 5. Boucle principale
    loop {
        // a) Lire les messages du thread lecteur
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ServerMsg::PowChallenge { tick, seed, resource_id, target_bits, .. } => {
                    println!("[~] Challenge reçu : resource={resource_id}");
                    miner_pool.submit(MineRequest { seed, tick, resource_id, agent_id, target_bits });
                }
                ServerMsg::Win { team } => {
                    println!("[!] Victoire : {team}");
                    std::process::exit(0);
                }
                _ => {}
            }
        }

        // b) Vérifier si le MinerPool a trouvé un nonce → envoyer PowSubmit
        if let Some(result) = miner_pool.try_recv() {
            println!("[+] Nonce trouvé : {}", result.nonce);
            let mut guard = ws.lock().unwrap();
            send_client_msg(&mut *guard, &ClientMsg::PowSubmit {
                tick: result.tick,
                resource_id: result.resource_id,
                nonce: result.nonce,
            });
        }

        // c) Consulter la stratégie → envoyer Move
        let move_opt = {
            let state = shared_state.lock().unwrap();
            strategy.next_move(&state)
        };
        if let Some((dx, dy)) = move_opt {
            let mut guard = ws.lock().unwrap();
            send_client_msg(&mut *guard, &ClientMsg::Move { dx, dy });
        }

        thread::sleep(Duration::from_millis(50));
    }
}

type WsStream = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

fn read_server_msg(ws: &mut WsStream) -> Option<ServerMsg> {
    match ws.read() {
        Ok(Message::Text(text)) => serde_json::from_str(&text).ok(),
        Ok(_) => None,
        Err(e) => {
            eprintln!("[!] Erreur WS lecture : {e}");
            None
        }
    }
}

fn send_client_msg(ws: &mut WsStream, msg: &ClientMsg) {
    let json = serde_json::to_string(msg).expect("sérialisation échouée");
    ws.send(Message::Text(json.into())).expect("envoi WS échoué");
}
