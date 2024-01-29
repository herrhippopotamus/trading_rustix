use actix_web::{
    get,
    middleware::Logger,
    post,
    web::{self, Data},
    App, HttpResponse, HttpServer, Responder, Result,
};
use env_logger::Env;

use rustix::envs::Envs;
use rustix::error::RustixErr;
use rustix::trading::{self, Trading};

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

#[post("/createPortfolio")]
async fn create_portfolio(
    data: Data<Trading>,
    portfolio: web::Json<trading::Portfolio>,
) -> Result<impl Responder> {
    let portfolio = data
        .create_portfolio(&portfolio.name, &portfolio.description)
        .await
        .map_err(|err| RustixErr::new(err, 500))?;
    Ok(web::Json(portfolio))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    HttpServer::new(|| {
        App::new()
            .app_data(Data::new(Trading::new(Envs::parse())))
            .wrap(Logger::default())
            .wrap(Logger::new("%a %{User-Agent}i"))
            .service(hello)
            .service(echo)
            .service(create_portfolio)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
