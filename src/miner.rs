// // ─── Partie 2 : Pool de mineurs ──────────────────────────────────────────────
// //
// // Objectif : créer un pool de N threads qui cherchent des nonces en parallèle.
// //
// // Concepts exercés : thread::spawn, mpsc::channel, Arc, move closures.
// //
// // Architecture :
// //
// //   Thread principal                        Threads mineurs (x N)
// //        |                                        |
// //        |── mpsc::Sender<MineRequest> ──────────>|  (challenges à résoudre)
// //        |                                        |
// //        |<── mpsc::Sender<MineResult> ──────────>|  (solutions trouvées)
// //        |                                        |
// //
// // Chaque thread mineur :
// //   1. Attend un MineRequest sur son channel
// //   2. Appelle pow::pow_search() avec un start_nonce différent
// //   3. Si un nonce est trouvé, envoie un MineResult
// //
// // ─────────────────────────────────────────────────────────────────────────────

// use std::thread;
// use uuid::Uuid;
// use std::sync::{Arc, Mutex};
// use std::sync::mpsc::channel;
// use std::sync::mpsc::Receiver;


// use crate::pow::pow_search;

// /// Requête de minage envoyée aux threads mineurs.
// #[derive(Debug, Clone)]
// pub struct MineRequest {
//     pub seed: String,
//     pub tick: u64,
//     pub resource_id: Uuid,
//     pub agent_id: Uuid,
//     pub target_bits: u8,
// }

// /// Résultat renvoyé par un mineur quand il trouve un nonce valide.
// #[derive(Debug, Clone)]
// pub struct MineResult {
//     pub tick: u64,
//     pub resource_id: Uuid,
//     pub nonce: u64,
// }

// // TODO: Définir la structure MinerPool.
// //
// // Elle doit contenir :
// //   - Le Sender pour envoyer des MineRequest aux threads
// //   - Le Receiver pour récupérer les MineResult
// //
// // Indice : les types sont :
// //   std::sync::mpsc::Sender<MineRequest>
// //   std::sync::mpsc::Receiver<MineResult>
// //
// pub struct MinerPool {
//     s: std::sync::mpsc::Sender<MineRequest>,
//     r: std::sync::mpsc::Receiver<MineResult>,
// }

// impl MinerPool {
//     /// Crée un pool de `n` threads mineurs.
//     ///
//     /// Chaque thread :
//     ///   1. Possède un Receiver<MineRequest> (partagé via Arc<Mutex<>>)
//     ///   2. Possède un Sender<MineResult> (cloné pour chaque thread)
//     ///   3. Boucle : recv() → pow_search() → send() si trouvé
//     ///
//     /// Indices :
//     ///   - Un seul Receiver existe par channel. Pour le partager entre N threads,
//     ///     il faut le wrapper dans Arc<Mutex<Receiver<MineRequest>>>.
//     ///   - Chaque thread clone le Arc pour accéder au Receiver.
//     ///   - pow::pow_search() prend un start_nonce et un batch_size.
//     ///     Utilisez rand::random::<u64>() comme start_nonce pour que chaque
//     ///     appel explore une zone différente.
//     ///   - Batch size recommandé : 100_000
//     ///
//     pub fn new(n: usize) -> Self {
//         let (request_tx, request_rx) = channel();
//         let (result_tx, result_rx) = channel();
//         let wrapped_request: Arc<Mutex<Receiver<MineRequest>>> = Arc::new(Mutex::new(request_rx));
//         for _ in 0..n {
//             let tx_wrapped = wrapped_request.clone();
//             let tx = result_tx.clone();
//             thread::spawn(move || {
//                 let req: MineRequest = tx_wrapped.lock().unwrap().recv().unwrap();
//                 let res_pow: Option<u64> = pow_search(
//                     &req.seed,
//                     req.tick,
//                     req.resource_id,
//                     req.agent_id,
//                     req.target_bits,
//                     rand::random::<u64>(),
//                     100_000,
//                 );
//                 let Some(res_pow) = res_pow else {
//                    panic!("Nonce non trouvé");
//                 };
//                 let res: MineResult = MineResult { tick: (req.tick), resource_id: (req.resource_id), nonce: (res_pow) };
//                 tx.send(res);
//             });
//         }
//         return MinerPool {
//             s: request_tx,
//             r: result_rx,
//         };
//         // Créer les 2 channels :
//         //   - (request_tx, request_rx) pour envoyer les challenges
//         //   - (result_tx, result_rx) pour recevoir les solutions
//         //
//         // Wrapper request_rx dans Arc<Mutex<>>
//         //
//         // Pour chaque thread (0..n) :
//         //   - Cloner le Arc<Mutex<Receiver<MineRequest>>>
//         //   - Cloner le result_tx
//         //   - thread::spawn(move || { ... boucle de minage ... })
//         //
//         // Retourner MinerPool { request_tx, result_rx }
//     }

//     /// Envoie un challenge de minage au pool.
//     pub fn submit(&self, request: MineRequest) {
//         self.s.send(request);
//     }

//     /// Tente de récupérer un résultat sans bloquer.
//     pub fn try_recv(&self) -> Option<MineResult> {
//         return Some(self.r.recv().unwrap());
//     }
// }



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