    use axum::{
        routing::{get, post},
        Router,
        response::Json,
        extract::State,
        http::StatusCode,
    };
    use serde::{Deserialize, Serialize};
    use std::sync::{Arc, Mutex};
    use std::net::SocketAddr;
    use tokio::sync::broadcast;
    use std::collections::HashMap;
    use tokio::net::TcpListener;

    // Message type for mesh communication
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MeshMessage {
        pub id: String,
        pub source: String,
        pub destination: String,
        pub payload: String,
        pub timestamp: u64,
    }

    // Shared state for the server
    #[derive(Clone)]
    struct AppState {
        // In-memory message store
        messages: Arc<Mutex<HashMap<String, MeshMessage>>>,
        // Channel for broadcasting messages to connected clients
        tx: broadcast::Sender<MeshMessage>,
    }

    #[tokio::main]
    async fn main() {
        // Initialize tracing for logging
        tracing_subscriber::fmt::init();

        // Create a broadcast channel for real-time updates
        let (tx, _) = broadcast::channel::<MeshMessage>(100);

        // Initialize shared state
        let state = AppState {
            messages: Arc::new(Mutex::new(HashMap::new())),
            tx,
        };

        // Build our application with routes
        let app = Router::new()
            .route("/api/messages", get(get_messages).post(send_message))
            .route("/api/messages/{id}", get(get_message))
            .route("/ws", get(websocket_handler))
            .with_state(state);

        // Run the server
        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
        println!("Mesh server running on http://{}", addr);
        
        let listener = TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    }

    // Handler to get all messages
    async fn get_messages(State(state): State<AppState>) -> Json<Vec<MeshMessage>> {
        let messages = state.messages.lock().unwrap();
        Json(messages.values().cloned().collect())
    }

    // Handler to get a specific message by ID
    async fn get_message(
        axum::extract::Path(id): axum::extract::Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<MeshMessage>, StatusCode> {
        let messages = state.messages.lock().unwrap();
        if let Some(message) = messages.get(&id) {
            Ok(Json(message.clone()))
        } else {
            Err(StatusCode::NOT_FOUND)
        }
    }

    // Handler to send a new message
    async fn send_message(
        State(state): State<AppState>,
        Json(payload): Json<MeshMessage>,
    ) -> StatusCode {
        let mut messages = state.messages.lock().unwrap();
        messages.insert(payload.id.clone(), payload.clone());
        
        // Broadcast the new message to all connected WebSocket clients
        let _ = state.tx.send(payload);
        
        StatusCode::CREATED
    }

    // WebSocket handler for real-time updates
    async fn websocket_handler(
        ws: axum::extract::ws::WebSocketUpgrade,
        State(state): State<AppState>,
    ) -> axum::response::Response {
        ws.on_upgrade(|socket| async move {
            // Handle WebSocket connection
            let mut rx = state.tx.subscribe();
            
            // For simplicity, we'll just print received messages
            // In a real application, you'd want to send them to the client
            while let Ok(msg) = rx.recv().await {
                println!("Received message via WebSocket: {:?}", msg);
            }
        })
    }
