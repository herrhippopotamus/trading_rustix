use actix_web::{
    get,
    middleware::Logger,
    post,
    web::{self, Data},
    App, HttpResponse, HttpServer, Responder, Result,
};
use env_logger::Env;
use serde::{Deserialize, Serialize};

use rustix::envs::Envs;
use rustix::error::RustixErr;
use rustix::trading::{self, Trading};

extern crate lazy_static;

#[derive(Deserialize)]
struct Filter {
    filter: String,
}
#[derive(Deserialize)]
struct Id {
    id: String,
}

#[derive(Serialize)]
struct Success {
    success: bool,
    error: Option<String>,
}
fn success() -> Success {
    Success {
        success: true,
        error: None,
    }
}

#[post("/tickers")]
async fn tickers(
    data: Data<Trading>,
    req: web::Json<trading::TickerFilter>,
) -> Result<HttpResponse> {
    let body = data
        .tickers(req.0)
        .await
        .map_err(|err| RustixErr::new(err, 500))?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .streaming(body))
}
#[post("/securityData")]
async fn security_data(
    data: Data<Trading>,
    req: web::Json<trading::TimeSeriesReq>,
) -> Result<HttpResponse> {
    let body = data
        .security_data(req.0)
        .await
        .map_err(|err| RustixErr::new(err, 500))?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .streaming(body))
}

#[post("/portfolio/create")]
async fn create_portfolio(
    data: Data<Trading>,
    req: web::Json<trading::Portfolio>,
) -> Result<impl Responder> {
    let resp = data
        .create_portfolio(&req.name, &req.description)
        .await
        .map_err(|err| RustixErr::new(err, 500))?;
    Ok(web::Json(resp))
}

#[post("/portfolio/buy")]
async fn buy_portfolio(
    data: Data<Trading>,
    req: web::Json<trading::PortfolioSecurity>,
) -> Result<impl Responder> {
    data.buy_security(req.0)
        .await
        .map_err(|err| RustixErr::new(err, 500))?;
    Ok(web::Json(success()))
}

#[post("/portfolio/sell")]
async fn sell_portfolio(
    data: Data<Trading>,
    req: web::Json<trading::PortfolioSecurity>,
) -> Result<impl Responder> {
    data.sell_security(req.0)
        .await
        .map_err(|err| RustixErr::new(err, 500))?;
    Ok(web::Json(success()))
}

#[get("/portfolio")]
async fn portfolio(data: Data<Trading>, query: web::Query<Id>) -> Result<impl Responder> {
    let resp = data
        .portfolio(query.0.id)
        .await
        .map_err(|err| RustixErr::new(err, 500))?;
    Ok(web::Json(resp))
}
#[get("/portfolios")]
async fn portfolios(data: Data<Trading>, query: web::Query<Filter>) -> Result<impl Responder> {
    let resp = data
        .portfolios(query.0.filter)
        .await
        .map_err(|err| RustixErr::new(err, 500))?;
    Ok(web::Json(resp))
}
#[post("/portfolio/profits")]
async fn portfolio_profits(
    data: Data<Trading>,
    req: web::Json<trading::SecurityProfitReq>,
) -> Result<impl Responder> {
    let resp = data
        .portfolio_profits(req.0)
        .await
        .map_err(|err| RustixErr::new(err, 500))?;
    Ok(web::Json(resp))
}
#[get("/portfolio/securities")]
async fn portfolio_securities(
    data: Data<Trading>,
    query: web::Query<Id>,
) -> Result<impl Responder> {
    let resp = data
        .portfolio_securities(query.0.id)
        .await
        .map_err(|err| RustixErr::new(err, 500))?;
    Ok(web::Json(resp))
}

#[post("/movements")]
async fn movements(
    data: Data<Trading>,
    req: web::Json<trading::MovementsReq>,
) -> Result<impl Responder> {
    let resp = data
        .movements(req.0)
        .await
        .map_err(|err| RustixErr::new(err, 500))?;
    Ok(web::Json(resp))
}
#[post("/correlatingTickers")]
async fn correlating_tickers(
    data: Data<Trading>,
    req: web::Json<trading::CorrelatingTickersReq>,
) -> Result<HttpResponse> {
    let body = data
        .correlating_tickers(req.0)
        .await
        .map_err(|err| RustixErr::new(err, 500))?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .streaming(body))
}
#[post("/mutualCorrelations")]
async fn mutual_correlations(
    data: Data<Trading>,
    req: web::Json<trading::CorrelReq>,
) -> Result<impl Responder> {
    let resp = data
        .mutual_correlations(req.0)
        .await
        .map_err(|err| RustixErr::new(err, 500))?;
    Ok(web::Json(resp))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let envs = Envs::parse();
    println!(
        "listening on {}:{} in mode '{}'",
        envs.host, envs.port, envs.mode,
    );
    env_logger::init_from_env(Env::default().default_filter_or(envs.mode));
    HttpServer::new(|| {
        App::new()
            .app_data(Data::new(Trading::new(Envs::parse())))
            .wrap(Logger::default())
            .wrap(Logger::new("%a %{User-Agent}i"))
            .service(
                web::scope("/api")
                    .service(tickers)
                    .service(portfolio)
                    .service(portfolios)
                    .service(create_portfolio)
                    .service(buy_portfolio)
                    .service(sell_portfolio)
                    .service(portfolio_profits)
                    .service(portfolio_securities)
                    .service(security_data)
                    .service(movements)
                    .service(correlating_tickers)
                    .service(mutual_correlations),
            )
    })
    .bind((envs.host, envs.port))?
    .run()
    .await
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use reqwest;
    use rustix::envs::Envs;
    use rustix::trading::{self};
    use serde;
    use std::time::Instant;
    use tokio;

    lazy_static! {
        static ref ENV: Envs = Envs::parse();
    }

    fn url(endpoint: &str) -> String {
        format!("http://{}:{}/api{}", ENV.host, ENV.port, endpoint)
    }
    async fn get_json<T>(endpoint: &str, query: Option<&[(&str, &str)]>) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = url(endpoint);
        let mut req = reqwest::Client::new().get(url);
        if let Some(query) = query {
            req = req.query(&query);
        }
        let res = req.send().await?.json().await?;
        Ok(res)
    }

    async fn post_json<T, ToJSON>(endpoint: &str, body: ToJSON) -> Result<T>
    where
        ToJSON: serde::ser::Serialize,
        T: serde::de::DeserializeOwned,
    {
        let url: String = url(endpoint);
        let body_json = serde_json::to_string(&body)?;
        println!("body_json for endpoint {}: {}", url, body_json);
        let res = reqwest::Client::new()
            .post(url)
            .body(body_json)
            .header("Content-Type", "application/json")
            .send()
            .await?
            .json()
            .await?;
        Ok(res)
    }

    #[tokio::test]
    async fn get_portfolios() -> Result<()> {
        let query = Some(&[("filter", "pack")][..]);
        let portfolios: Vec<trading::Portfolio> = get_json("/portfolios", query).await?;

        assert!(portfolios.len() > 0);
        Ok(())
    }

    #[tokio::test]
    async fn tickers() -> Result<()> {
        let req_body = trading::TickerFilter {
            ttype: 0,
            filter: Some("aapl".to_string()),
            limit: Some(10),
            traded_within_past_n_days: None,
        };
        let ts: Vec<trading::Ticker> = post_json("/tickers", req_body).await?;
        println!("streamed {} tickers", ts.len());
        assert!(ts.len() > 0);
        Ok(())
    }

    #[tokio::test]
    async fn security_data() -> Result<()> {
        let start = Instant::now();
        let req_body = trading::TimeSeriesReq {
            ticker: trading::BasicTicker {
                ticker: "AAPL".to_string(),
                security_type: 0,
            },
            from: "2024-01-01".to_string(),
            until: "2024-01-31".to_string(),
        };
        let ts: Vec<trading::TimeSeriesData> = post_json("/securityData", req_body).await?;
        println!(
            "streamed {} security data in {:?}",
            ts.len(),
            start.elapsed()
        );
        assert!(ts.len() > 100);
        Ok(())
    }
}
