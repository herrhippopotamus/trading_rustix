use crate::envs::Envs;
use anyhow::Result;
use dataloader::data_loader_client::DataLoaderClient;
use dataloader::{
    self as db_proto, BasicTicker, Correl, CorrelReq, CorrelTickersReq, DetailedCorrel, Movement,
    MovementsReq, MutualCorrel, Ticker, TickerFilter, TimeSeriesData, TimeSeriesReq,
};
// use futures_util::TryFutureExt;
use serde::{Deserialize, Serialize};
use std::clone::Clone;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use tokio::sync::mpsc::{self, Receiver};
use tokio_stream::StreamExt;
use tonic::transport::Channel;
use tonic::Streaming;

#[derive(Debug)]
pub struct StreamError {
    src: String,
}

impl Error for StreamError {}
impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.src)
    }
}
impl StreamError {
    pub fn new<T: ToString>(s: T) -> StreamError {
        StreamError { src: s.to_string() }
    }
}

impl From<String> for StreamError {
    fn from(t: String) -> Self {
        Self { src: t }
    }
}
impl From<StreamError> for String {
    fn from(e: StreamError) -> String {
        e.src
    }
}
pub mod dataloader {
    tonic::include_proto!("dataloader");
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JSONTicker {
    ticker: String,
    name: Option<String>,
    security_type: i32,
    #[serde(flatten)]
    custom_fields: Option<HashMap<String, String>>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct JSONBasicTicker {
    ticker: String,
    security_type: i32,
}

impl From<Ticker> for JSONTicker {
    fn from(t: Ticker) -> Self {
        Self {
            ticker: t.ticker,
            name: Some(t.name),
            security_type: t.security_type,
            custom_fields: Some(t.custom_fields),
        }
    }
}
impl From<JSONBasicTicker> for BasicTicker {
    fn from(t: JSONBasicTicker) -> Self {
        Self {
            ticker: t.ticker,
            security_type: t.security_type,
        }
    }
}
impl Clone for JSONBasicTicker {
    fn clone(&self) -> Self {
        Self {
            ticker: self.ticker.to_string(),
            security_type: self.security_type,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JSONTimeSeriesData {
    date: String,
    values: HashMap<String, f64>,
}
impl From<TimeSeriesData> for JSONTimeSeriesData {
    fn from(s: TimeSeriesData) -> Self {
        Self {
            date: s.date,
            values: s.values,
        }
    }
}

#[derive(Serialize)]
pub struct JSONMovement {
    ticker: JSONTicker,
    performance: f64,
    average: f64,
    volume: f64,
    variance: f64,
    stddev: f64,
    date: String,
    period: i32,
}
impl From<Movement> for JSONMovement {
    fn from(m: Movement) -> Self {
        Self {
            ticker: JSONTicker {
                ticker: m.ticker,
                security_type: m.security_type,
                name: Some(m.name),
                custom_fields: None,
            },
            performance: m.performance,
            average: m.average,
            volume: m.volume,
            variance: m.variance,
            stddev: m.stddev,
            date: m.date,
            period: m.period,
        }
    }
}

#[derive(Serialize)]
pub struct JSONDetailedCorrel {
    ticker0: JSONTicker,
    ticker1: JSONTicker,
    date: String,
    period: i32,
    correlation: f64,
}

#[derive(Serialize)]
pub struct JSONMutualCorrelation {
    ticker: JSONTicker,
    correlations: Vec<JSONDetailedCorrel>,
    volatility: f64,
    stddev: f64,
    performance: f64,
    volume: f64,
}
impl TryFrom<MutualCorrel> for JSONMutualCorrelation {
    type Error = StreamError;
    fn try_from(c: MutualCorrel) -> Result<Self, Self::Error> {
        Ok(Self {
            ticker: c
                .ticker
                .ok_or_else(|| StreamError::from("MutualCorrel - invalid ticker".to_string()))?
                .into(),
            correlations: c
                .correlations
                .into_iter()
                .map(|c| c.try_into())
                .collect::<Result<Vec<JSONDetailedCorrel>, Self::Error>>()?,
            volatility: c.volatility,
            stddev: c.stddev,
            performance: c.performance,
            volume: c.volume,
        })
    }
}
impl TryFrom<DetailedCorrel> for JSONDetailedCorrel {
    type Error = StreamError;
    fn try_from(c: DetailedCorrel) -> Result<Self, Self::Error> {
        Ok(Self {
            ticker0: c.ticker0.unwrap().into(),
            ticker1: c.ticker1.unwrap().into(),
            date: c.date,
            period: c.period,
            correlation: c.correl,
        })
    }
}
impl Clone for JSONTicker {
    fn clone(&self) -> Self {
        Self {
            ticker: self.ticker.to_string(),
            name: self.name.clone(),
            security_type: self.security_type,
            custom_fields: self.custom_fields.clone(),
        }
    }
}

pub type Portfolios = Vec<Portfolio>;
#[derive(Deserialize, Serialize, Debug)]
pub struct Portfolio {
    pub id: String,
    pub name: String,
    pub description: String,
}

impl From<db_proto::PortfolioMeta> for Portfolio {
    fn from(p: db_proto::PortfolioMeta) -> Self {
        Self {
            id: p.id,
            name: p.name,
            description: p.description,
        }
    }
}
impl From<db_proto::PortfolioMetas> for Vec<Portfolio> {
    fn from(p: db_proto::PortfolioMetas) -> Self {
        p.portfolios.into_iter().map(|p| p.into()).collect()
    }
}

pub type PortfolioSecurities = Vec<PortfolioSecurity>;
#[derive(Serialize, Debug)]
pub struct PortfolioSecurity {
    portfolio_id: String,
    security_type: i32,
    ticker: String,
    volume: f64,
    purchase_date: String,
    sell_date: String,
}

impl From<db_proto::PortfolioSecurity> for PortfolioSecurity {
    fn from(p: db_proto::PortfolioSecurity) -> Self {
        Self {
            portfolio_id: p.portfolio_id,
            security_type: p.security_type,
            ticker: p.ticker,
            volume: p.volume,
            purchase_date: p.purchase_date,
            sell_date: p.sell_date,
        }
    }
}
impl From<PortfolioSecurity> for db_proto::PortfolioSecurity {
    fn from(p: PortfolioSecurity) -> Self {
        Self {
            portfolio_id: p.portfolio_id,
            security_type: p.security_type,
            ticker: p.ticker,
            volume: p.volume,
            purchase_date: p.purchase_date,
            sell_date: p.sell_date,
        }
    }
}
// impl From<Vec<PortfolioSecurity>> for Vec<db_proto::PortfolioSecurities> {
//     fn from(securities: Vec<PortfolioSecurity>) -> Self {
//         Self {
//             securities: securities.into_iter().map(|p| p.into()).collect(),
//         }
//     }
// }
pub type SecurityProfits = Vec<SecurityProfit>;
#[derive(Serialize, Debug)]
pub struct SecurityProfit {
    ticker: String,
    security_type: i32,
    purchase_date: String,
    until: String,
    purchase_price: f64,
    profit_per_share: f64,
    volume: f64,
    total_profit: f64,
}

impl From<db_proto::SecurityProfit> for SecurityProfit {
    fn from(p: db_proto::SecurityProfit) -> Self {
        Self {
            ticker: p.ticker,
            security_type: p.security_type,
            purchase_date: p.purchase_date,
            until: p.until,
            purchase_price: p.purchase_price,
            profit_per_share: p.profit_per_share,
            volume: p.volume,
            total_profit: p.total_profit,
        }
    }
}
impl From<db_proto::SecurityProfits> for Vec<SecurityProfit> {
    fn from(p: db_proto::SecurityProfits) -> Self {
        p.profits.into_iter().map(|p| p.into()).collect()
    }
}

pub type JSONMovements = Vec<JSONMovement>;

#[derive(Serialize, Debug)]
pub struct JSONCorrelatingTickers {
    tickers: Vec<JSONTicker>,
    correlation: f64,
    date: String,
    period: i32,
    volume0: f64,
    volume1: f64,
}
impl From<Correl> for JSONCorrelatingTickers {
    fn from(c: Correl) -> Self {
        let ticker0 = c.ticker0.unwrap();
        let ticker1 = c.ticker1.unwrap();
        Self {
            tickers: vec![
                JSONTicker {
                    ticker: ticker0.ticker,
                    security_type: ticker0.security_type,
                    name: None,
                    custom_fields: None,
                },
                JSONTicker {
                    ticker: ticker1.ticker,
                    security_type: ticker1.security_type,
                    name: None,
                    custom_fields: None,
                },
            ],
            correlation: c.correl,
            date: c.date,
            period: c.period,
            volume0: c.volume0,
            volume1: c.volume1,
        }
    }
}

pub struct Trading {
    db_loader_host: String,
    db_loader_port: u16,
}

// async fn stream_to_vec<Src>(mut stream: Streaming<Src>) -> Result<Vec<Src>>
// where
//     Src: 'static + Send,
//     // Tar: From<Src> + Send + 'static,
// {
//     let mut vec = Vec::new();
//     while let Some(entry) = stream.next().await {
//         vec.push(entry?);
//     }
//     Ok(vec)
// }
impl Trading {
    pub fn new(envs: Envs) -> Trading {
        Trading {
            db_loader_host: envs.db_loader_host,
            db_loader_port: envs.db_loader_port,
        }
    }
    async fn client(&self) -> Result<DataLoaderClient<Channel>> {
        Ok(DataLoaderClient::connect(format!(
            "http://{}:{}",
            self.db_loader_host, self.db_loader_port,
        ))
        .await?)
    }
    pub async fn get_trading_data<Src, Tar>(
        &self,
        mut stream: Streaming<Src>,
    ) -> Result<Receiver<Tar>>
    where
        Src: 'static + Send,
        Tar: From<Src> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel::<Tar>(100);

        tokio::spawn(async move {
            while let Some(entry) = stream.next().await {
                let entry = entry
                    .map_err(|err| format!("gRPC-error: received invalid entry: {:?}", err))?;
                let tar = entry.into();
                tx.send(tar)
                    .await
                    .map_err(|_| "gRPC-error - sending entry failed".to_string())?;
            }
            Result::<(), String>::Ok(())
        });
        Ok(rx)
    }
    pub async fn get_tickers(
        &self,
        ttype: i32,
        filter: Option<&str>,
        limit: u32,
    ) -> Result<Receiver<JSONTicker>> {
        let stream = self
            .client()
            .await?
            .get_tickers(tonic::Request::new(TickerFilter {
                ticker_type: ttype,
                filter: filter.unwrap_or("").to_string(),
                limit,
                traded_within_past_n_days: 10,
            }))
            .await?
            .into_inner();

        self.get_trading_data(stream).await
    }
    pub async fn get_movements(
        &self,
        security_type: u32,
        sort_by: u32,
        until: String,
        period: u32,
        limit: u32,
        min_volume: u64,
    ) -> Result<JSONMovements> {
        Ok(self
            .client()
            .await?
            .get_movements(tonic::Request::new(MovementsReq {
                security_type: security_type as i32,
                until,
                period: period as i32,
                sort_by: sort_by as i32,
                limit,
                min_volume,
            }))
            .await?
            .into_inner()
            .movements
            .into_iter()
            .map(|m| m.into())
            .collect())
    }
    pub async fn get_correlating_tickers(
        &self,
        until: String,
        period: u32,
        limit: u32,
    ) -> Result<Receiver<JSONCorrelatingTickers>> {
        println!(
            "in get_correlating_tickers: {}, {}, {}",
            until, period, limit
        );
        let (tx, rx) = mpsc::channel(if limit > 0 { limit as usize } else { 100 });
        let mut stream = self
            .client()
            .await?
            .get_correlating_tickers(tonic::Request::new(CorrelTickersReq {
                until,
                period: period as i32,
                limit,
            }))
            .await?
            .into_inner();
        tokio::spawn(async move {
            while let Some(ticker) = stream.next().await {
                tx.send(ticker?.into()).await?;
            }
            Result::<()>::Ok(())
        });
        Ok(rx)
    }
    pub async fn get_mutual_correlations(
        &self,
        tickers: &Vec<JSONBasicTicker>,
        until: Option<&str>,
        period: u32,
    ) -> Result<Vec<JSONMutualCorrelation>> {
        let mut client = self.client().await?;
        let mutual_correls = client
            .get_mutual_correlations(tonic::Request::new(CorrelReq {
                tickers: tickers
                    .into_iter()
                    .map(|t: &JSONBasicTicker| t.clone().into())
                    .collect(),
                until: until.unwrap_or("").to_string(),
                period: period as i32,
            }))
            .await?
            .into_inner();
        let mutual_correls = mutual_correls
            .correls
            .into_iter()
            .map(|mutual| mutual.try_into())
            .collect::<Result<Vec<_>, StreamError>>()?;
        Ok(mutual_correls)
    }
    pub async fn get_security_data(
        &self,
        ticker: BasicTicker,
        from: &str,
        until: &str,
        intraday: bool,
    ) -> Result<Receiver<JSONTimeSeriesData>> {
        let (tx, rx) = mpsc::channel(100);
        let mut stream = self
            .client()
            .await?
            .get_security_data(tonic::Request::new(TimeSeriesReq {
                ticker: Some(ticker),
                from_date: from.to_string(),
                until_date: until.to_string(),
                intraday: intraday,
            }))
            .await?
            .into_inner();

        tokio::spawn(async move {
            while let Some(stock) = stream.next().await {
                tx.send(stock?.into()).await?;
            }
            Result::<()>::Ok(())
        });
        Ok(rx)
    }
    pub async fn portfolios(&self, filter: &str) -> Result<Portfolios> {
        let mut client = self.client().await?;
        Ok(client
            .get_portfolios(tonic::Request::new(db_proto::PortfolioReq {
                filter: filter.to_string(),
            }))
            .await?
            .into_inner()
            .into())
    }
    pub async fn portfolio_securities(&self, portfolio_id: &str) -> Result<PortfolioSecurities> {
        let mut client = self.client().await?;
        Ok(client
            .get_portfolio_securities(tonic::Request::new(db_proto::Id {
                id: portfolio_id.to_string(),
            }))
            .await?
            .into_inner()
            .securities
            .into_iter()
            .map(|s| s.into())
            .collect())
    }
    pub async fn portfolio_profits(
        &self,
        req: db_proto::SecurityProfitReq,
    ) -> Result<SecurityProfits> {
        let mut client = self.client().await?;
        Ok(client
            .get_portfolio_profits(tonic::Request::new(req))
            .await?
            .into_inner()
            .into())
    }
    pub async fn create_portfolio(&self, name: &str, description: &str) -> Result<Portfolio> {
        let mut client = self.client().await?;
        Ok(client
            .create_portfolio(tonic::Request::new(db_proto::CreatePortfolioReq {
                name: name.to_string(),
                description: description.to_string(),
            }))
            .await?
            .into_inner()
            .into())
    }
    pub async fn buy_security(&self, security: PortfolioSecurity) -> Result<()> {
        let mut client = self.client().await?;
        client
            .buy_security(tonic::Request::new(security.into()))
            .await?;
        Ok(())
    }
    pub async fn sell_security(&self, security: PortfolioSecurity) -> Result<()> {
        let mut client = self.client().await?;
        client
            .sell_security(tonic::Request::new(security.into()))
            .await?;
        Ok(())
    }
}
