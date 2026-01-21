use actix::{Actor, StreamHandler, AsyncContext, Handler, Message, Context, Recipient, Addr};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
// Removi Arc e Mutex que não estavam sendo usados
use uuid::Uuid;

// --- ESTRUTURAS DE DADOS ---

// A mensagem que viaja pela rede (JSON)
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Position {
    x: f32,
    y: f32,
}

// Mensagem interna do Rust para o Ator
#[derive(Message)]
#[rtype(result = "()")]
struct WsMessage(String);

// --- O HUB (LOBBY) ---
// Ele guarda a lista de todo mundo que está online
struct Lobby {
    sessions: HashMap<Uuid, Recipient<WsMessage>>,
}

impl Lobby {
    fn new() -> Self {
        Lobby { sessions: HashMap::new() }
    }
}

// Transforma o Lobby em um Ator
impl Actor for Lobby {
    type Context = Context<Self>;
}

// Mensagem para entrar no Lobby
#[derive(Message)]
#[rtype(result = "()")]
struct Connect {
    id: Uuid,
    addr: Recipient<WsMessage>,
}

impl Handler<Connect> for Lobby {
    type Result = ();

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) {
        self.sessions.insert(msg.id, msg.addr);
        println!("Novo usuário conectado! Total: {}", self.sessions.len());
    }
}

// Mensagem para sair do Lobby
#[derive(Message)]
#[rtype(result = "()")]
struct Disconnect {
    id: Uuid,
}

impl Handler<Disconnect> for Lobby {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        self.sessions.remove(&msg.id);
    }
}

// Mensagem de Broadcast (Espalhar a fofoca)
#[derive(Message)]
#[rtype(result = "()")]
struct Broadcast {
    id: Uuid,    // Quem mandou (para não mandar de volta pra ele mesmo se não quiser)
    msg: String, // O JSON com a posição
}

impl Handler<Broadcast> for Lobby {
    type Result = ();

    fn handle(&mut self, msg: Broadcast, _: &mut Context<Self>) {
        // Manda para TODO MUNDO (inclusive quem enviou, para garantir sincronia total)
        for (_, addr) in &self.sessions {
            let _ = addr.do_send(WsMessage(msg.msg.clone()));
        }
    }
}

// --- A SESSÃO INDIVIDUAL (Cada Aba do Navegador) ---
struct MyWs {
    id: Uuid,
    lobby_addr: Addr<Lobby>,
}

impl Actor for MyWs {
    type Context = ws::WebsocketContext<Self>;

    // Quando a conexão começa
    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        self.lobby_addr.do_send(Connect {
            id: self.id,
            addr: addr.recipient(),
        });
    }

    // Quando a conexão cai
    fn stopping(&mut self, _: &mut Self::Context) -> actix::Running {
        self.lobby_addr.do_send(Disconnect { id: self.id });
        actix::Running::Stop
    }
}

// Trata as mensagens que vêm do Frontend
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                // MUDANÇA AQUI:
                // Em vez de só repassar, a gente cria um JSON novo:
                // { "id": "uuid-do-usuario", "data": {x: 10, y: 20} }
                let msg_with_id = format!(r#"{{"id": "{}", "data": {} }}"#, self.id, text);
                
                self.lobby_addr.do_send(Broadcast {
                    id: self.id,
                    msg: msg_with_id,
                });
            }
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            _ => (),
        }
    }
}

// Trata as mensagens que vêm do Lobby (Broadcast) para enviar pro Frontend
impl Handler<WsMessage> for MyWs {
    type Result = ();

    fn handle(&mut self, msg: WsMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

// --- ROTA DE ENTRADA ---
async fn ws_index(
    req: HttpRequest,
    stream: web::Payload,
    lobby: web::Data<Addr<Lobby>>,
) -> Result<HttpResponse, Error> {
    let ws = MyWs {
        id: Uuid::new_v4(),
        lobby_addr: lobby.get_ref().clone(),
    };
    ws::start(ws, &req, stream)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Inicia o Lobby
    let lobby = Lobby::new().start();
    let lobby_data = web::Data::new(lobby);

    println!("📡 Servidor Sync rodando em http://localhost:8080");

    HttpServer::new(move || {
        App::new()
            .app_data(lobby_data.clone())
            .route("/ws", web::get().to(ws_index)) // Rota do WebSocket
            .service(actix_files::Files::new("/", "./static").index_file("index.html"))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}