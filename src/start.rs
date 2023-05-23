use rand::Rng;
use hyper::Uri;

use axum::{
    http::Request,
    response::Redirect,
    body::Body, 
    routing::get,
};

use std::{
    thread,
    net::TcpListener,
    sync::{Arc, Mutex},
    process::{Command, Stdio}
};

pub async fn valve_start(host: String, port: u16, n_threads: u16) {
    //let cli_args: Cli = argh::from_env();
    //println!("{cli_args:?}");
    let axum_host = Arc::new(host);
    let axum_port = port;
    let n_threads = n_threads;
    // TODO spawn new threads if need be

    // specify the number of Plumber APIs to spawn
    let num_threads = n_threads;
    let ports = Arc::new(Mutex::new(
        (0..num_threads)
            .map(|_| generate_random_port(axum_host.as_str()))
            .collect::<Vec<u16>>()
            .into_iter()
            .cycle(),
    ));

    // start R and print R_HOME
    // All threads that are spawned for a plumber API are going to be blocked
    // Those threads can't ever be used to return a value. So joining is impossible
    for _ in 0..num_threads {
        //let ports_clone = Arc::clone(&ports);
        let port_i = ports.lock().unwrap().next().unwrap();
        let axum_host = axum_host.clone();
        let _handle = thread::spawn(move || {
            let port = port_i;
            println!("Spawning Plumber API at {axum_host}:{port}");
            spawn_plumber(&axum_host, port);
        });
    }

    // Access the ports data
    //let ports_data = ports.clone();
    println!("Spawned ports: {ports:?}");

    // first port will be used to host docs
    let first_port = ports.clone().lock().unwrap().next().unwrap();

    // Create the Axum application
    // clone the host to be passed into the handlers these should be done via a 
    // State approach, though.
    let ah_doc = axum_host.clone();
    let ah_reroute = axum_host.clone();
    let app = axum::Router::new()
        .route(
            "/__docs__",
            get(move || {
                let axum_host = ah_doc;
                async move {
                    // Create the docs path using the cloned value
                    let doc_path = format!("http://{}:{first_port}/__docs__/", axum_host.as_str());
                    Redirect::to(doc_path.as_str())
                }
            }),
        )
        .route(
            "/*key",
            axum::routing::any(move |req: Request<Body>| {
                let axum_host = ah_reroute;
                async move {
                    // select the next port
                    let port = ports.lock().unwrap().next().unwrap();
                    let ruri = req.uri(); // get the URI
                    let mut uri = ruri.clone().into_parts(); // clone 
                    // change URI to random port from above
                    uri.authority = Some(format!("{}:{port}", axum_host.as_str()).as_str().parse().unwrap());
                    // TODO enable https or other schemes
                    uri.scheme = Some("http".parse().unwrap());
                    let new_uri = Uri::from_parts(uri).unwrap().to_string();
                    Redirect::temporary(new_uri.as_str())
                }
            }),
        );
    // Start the Axum server
    let full_axum_host = format!("{axum_host}:{axum_port}");
    axum::Server::bind(&full_axum_host.as_str().parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}



// spawn plumber
fn spawn_plumber(host: &str, port: u16) {
    let mut _output = Command::new("R")
        .arg("-e")
        .arg(format!("plumber::plumb('plumber.R')$run(host = '{host}', port = {port})"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start R process");
}

// from chatGPT
// these functions generate random
fn generate_random_port(host: &str) -> u16 {
    let mut rng = rand::thread_rng();
    loop {
        let port: u16 = rng.gen_range(1024..=65535);
        if is_port_available(host, port) {
            return port;
        }
    }
}
// checks to see if the port is available
fn is_port_available(host: &str, port: u16) -> bool {
    match TcpListener::bind(format!("{host}:{port}")) {
        Ok(listener) => {
            // The port is available, so we close the listener and return true
            drop(listener);
            true
        }
        Err(_) => false, // The port is not available
    }
}
