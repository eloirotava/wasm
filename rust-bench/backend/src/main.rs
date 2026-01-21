use actix_files::Files;
use actix_web::{web, App, HttpServer, HttpResponse, Responder};
use core_algo::heavy_computation;

// Rota que roda NATIVO (no servidor Linux)
async fn run_native(path: web::Path<usize>) -> impl Responder {
    let size = path.into_inner();
    println!("Iniciando Nativo (ELF) com tamanho {}...", size);
    
    // web::block para não travar a thread async do servidor
    let result = web::block(move || heavy_computation(size)).await.unwrap();
    
    match result {
        Ok(msg) => HttpResponse::Ok().body(msg),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Servidor rodando em http://127.0.0.1:8080");
    HttpServer::new(|| {
        App::new()
            .route("/api/native/{size}", web::get().to(run_native))
            .service(Files::new("/", "./static").index_file("index.html"))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}