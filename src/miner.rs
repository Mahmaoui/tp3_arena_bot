use std::thread;
use uuid::Uuid;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;

use crate::pow::pow_search;

#[derive(Debug, Clone)]
pub struct MineRequest {
    pub seed: String,
    pub tick: u64,
    pub resource_id: Uuid,
    pub agent_id: Uuid,
    pub target_bits: u8,
}

#[derive(Debug, Clone)]
pub struct MineResult {
    pub tick: u64,
    pub resource_id: Uuid,
    pub nonce: u64,
}

pub struct MinerPool {
    s: std::sync::mpsc::Sender<MineRequest>,
    r: std::sync::mpsc::Receiver<MineResult>,
}

impl MinerPool {
    pub fn new(n: usize) -> Self {
        let (request_tx, request_rx) = channel();
        let (result_tx, result_rx) = channel();
        let wrapped_request: Arc<Mutex<Receiver<MineRequest>>> = Arc::new(Mutex::new(request_rx));

        for _ in 0..n {
            let rx_clone = wrapped_request.clone();
            let tx = result_tx.clone();
            thread::spawn(move || {
                loop {
                    // Attendre le prochain challenge
                    let req = rx_clone.lock().unwrap().recv().unwrap();

                    // Chercher un nonce, relancer avec un nouveau start si pas trouvé
                    loop {
                        let start = rand::random::<u64>();
                        if let Some(nonce) = pow_search(
                            &req.seed, req.tick, req.resource_id,
                            req.agent_id, req.target_bits, start, 100_000,
                        ) {
                            let _ = tx.send(MineResult {
                                tick: req.tick,
                                resource_id: req.resource_id,
                                nonce,
                            });
                            break;
                        }
                    }
                }
            });
        }

        MinerPool { s: request_tx, r: result_rx }
    }

    pub fn submit(&self, request: MineRequest) {
        let _ = self.s.send(request);
    }

    /// Non-bloquant : retourne None s'il n'y a pas encore de résultat.
    pub fn try_recv(&self) -> Option<MineResult> {
        self.r.try_recv().ok()
    }
}