use crate::envs::Envs;
use anyhow::Result;
use bytes::Bytes;
use dataloader::data_loader_client::DataLoaderClient;
use dataloader::{self as db_proto};
use serde::{Deserialize, Serialize};
use std::clone::Clone;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
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
pub struct TickerFilter {
    #[serde(rename = "security_type")]
    pub ttype: i32,
    #[serde(default)]
    pub filter: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub traded_within_past_n_days: Option<u32>,
}

impl From<TickerFilter> for db_proto::TickerFilter {
    fn from(t: TickerFilter) -> Self {
        Self {
            ticker_type: t.ttype,
            filter: t.filter.unwrap_or("".to_string()),
            limit: t.limit.unwrap_or(100),
            traded_within_past_n_days: t.traded_within_past_n_days.unwrap_or(10),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MovementsReq {
    pub security_type: u32,
    pub sort_by: u32,
    pub until: String,
    pub period: u32,
    pub limit: u32,
    pub min_volume: u64,
}
impl From<MovementsReq> for db_proto::MovementsReq {
    fn from(m: MovementsReq) -> Self {
        Self {
            security_type: m.security_type as i32,
            sort_by: m.sort_by as i32,
            until: m.until,
            period: m.period as i32,
            limit: m.limit,
            min_volume: m.min_volume,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CorrelatingTickersReq {
    pub until: String,
    pub period: u32,
    pub limit: u32,
}
impl From<CorrelatingTickersReq> for db_proto::CorrelTickersReq {
    fn from(c: CorrelatingTickersReq) -> Self {
        Self {
            until: c.until,
            period: c.period as i32,
            limit: c.limit,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Ticker {
    ticker: String,
    name: Option<String>,
    security_type: i32,
    #[serde(flatten)]
    custom_fields: Option<HashMap<String, String>>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct BasicTicker {
    pub ticker: String,
    pub security_type: i32,
}

impl From<db_proto::Ticker> for Ticker {
    fn from(t: db_proto::Ticker) -> Self {
        Self {
            ticker: t.ticker,
            name: Some(t.name),
            security_type: t.security_type,
            custom_fields: Some(t.custom_fields),
        }
    }
}
impl From<BasicTicker> for db_proto::BasicTicker {
    fn from(t: BasicTicker) -> Self {
        Self {
            ticker: t.ticker,
            security_type: t.security_type,
        }
    }
}
impl Clone for BasicTicker {
    fn clone(&self) -> Self {
        Self {
            ticker: self.ticker.to_string(),
            security_type: self.security_type,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TimeSeriesData {
    date: String,
    values: HashMap<String, f64>,
}
impl From<db_proto::TimeSeriesData> for TimeSeriesData {
    fn from(s: db_proto::TimeSeriesData) -> Self {
        Self {
            date: s.date,
            values: s.values,
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct TimeSeriesReq {
    pub ticker: BasicTicker,
    pub from: String,
    pub until: String,
}
impl From<TimeSeriesReq> for db_proto::TimeSeriesReq {
    fn from(t: TimeSeriesReq) -> Self {
        Self {
            ticker: Some(t.ticker.into()),
            from_date: t.from,
            until_date: t.until,
            intraday: true,
        }
    }
}

#[derive(Serialize)]
pub struct Movement {
    pub ticker: Ticker,
    pub performance: f64,
    pub average: f64,
    pub volume: f64,
    pub variance: f64,
    pub stddev: f64,
    pub date: String,
    pub period: i32,
}
impl From<db_proto::Movement> for Movement {
    fn from(m: db_proto::Movement) -> Self {
        Self {
            ticker: Ticker {
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
pub struct DetailedCorrel {
    ticker0: Ticker,
    ticker1: Ticker,
    date: String,
    period: i32,
    correlation: f64,
}

#[derive(Serialize)]
pub struct MutualCorrel {
    ticker: Ticker,
    correlations: Vec<DetailedCorrel>,
    volatility: f64,
    stddev: f64,
    performance: f64,
    volume: f64,
}
impl TryFrom<db_proto::MutualCorrel> for MutualCorrel {
    type Error = StreamError;
    fn try_from(c: db_proto::MutualCorrel) -> Result<Self, Self::Error> {
        Ok(Self {
            ticker: c
                .ticker
                .ok_or_else(|| StreamError::from("MutualCorrel - invalid ticker".to_string()))?
                .into(),
            correlations: c
                .correlations
                .into_iter()
                .map(|c| c.try_into())
                .collect::<Result<Vec<DetailedCorrel>, Self::Error>>()?,
            volatility: c.volatility,
            stddev: c.stddev,
            performance: c.performance,
            volume: c.volume,
        })
    }
}
impl TryFrom<db_proto::DetailedCorrel> for DetailedCorrel {
    type Error = StreamError;
    fn try_from(c: db_proto::DetailedCorrel) -> Result<Self, Self::Error> {
        Ok(Self {
            ticker0: c.ticker0.unwrap().into(),
            ticker1: c.ticker1.unwrap().into(),
            date: c.date,
            period: c.period,
            correlation: c.correl,
        })
    }
}
impl Clone for Ticker {
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
#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
pub struct Security {
    security_type: i32,
    ticker: String,
    volume: f64,
    purchase_date: Option<String>,
    sell_date: Option<String>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct SecurityProfitReq {
    pub util: String,
    pub parition: i32,
    pub securities: Vec<Security>,
}

impl From<SecurityProfitReq> for db_proto::SecurityProfitReq {
    fn from(req: SecurityProfitReq) -> Self {
        Self {
            until: req.util,
            partition: req.parition,
            securities: req
                .securities
                .into_iter()
                .map(|s| db_proto::security_profit_req::Security {
                    security_type: s.security_type,
                    ticker: s.ticker,
                    volume: s.volume,
                    purchase_date: s.purchase_date.unwrap_or("".to_string()),
                    sell_date: s.sell_date,
                })
                .collect(),
        }
    }
}

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

pub type Movements = Vec<Movement>;

#[derive(Serialize, Debug)]
pub struct CorrelatingTickers {
    tickers: Vec<Ticker>,
    correlation: f64,
    date: String,
    period: i32,
    volume0: f64,
    volume1: f64,
}
impl From<db_proto::Correl> for CorrelatingTickers {
    fn from(c: db_proto::Correl) -> Self {
        let ticker0 = c.ticker0.unwrap();
        let ticker1 = c.ticker1.unwrap();
        Self {
            tickers: vec![
                Ticker {
                    ticker: ticker0.ticker,
                    security_type: ticker0.security_type,
                    name: None,
                    custom_fields: None,
                },
                Ticker {
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

pub type ActixStreamItem = Result<Bytes, StreamError>;
pub type ActixStream = ReceiverStream<ActixStreamItem>;

async fn gprc_to_stream<Src, ToJSON>(mut stream: Streaming<Src>, to_json: ToJSON) -> ActixStream
where
    Src: Send + 'static,
    ToJSON: Send + 'static + Fn(Src) -> Result<String>,
{
    let (tx, rx) = mpsc::channel::<ActixStreamItem>(100);

    tokio::spawn(async move {
        tx.send(Ok(Bytes::from("["))).await?;
        let mut entries_count = 0;
        while let Some(entry) = stream.next().await {
            if entries_count > 0 {
                tx.send(Ok(Bytes::from(","))).await?;
            }
            entries_count += 1;
            let entry = entry
                .map(|entry| to_json(entry))
                .map(|js| Bytes::from(js.unwrap()))
                .map_err(|err| StreamError::from(err.to_string()));
            if let Err(err) = tx.send(entry).await {
                println!("gRPC-error: sending entry failed: {:?}", err);
            }
        }
        tx.send(Ok(Bytes::from("]"))).await
    });
    ReceiverStream::new(rx)
}

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

    pub async fn tickers(&self, filter: TickerFilter) -> Result<ActixStream> {
        let stream = self
            .client()
            .await?
            .get_tickers(tonic::Request::new(filter.into()))
            .await?
            .into_inner();

        let to_json = |t: db_proto::Ticker| -> Result<String> {
            let t: Ticker = t.into();
            let js = serde_json::to_string(&t).unwrap();
            Ok(js)
        };
        Ok(gprc_to_stream(stream, to_json).await)
    }
    pub async fn movements(&self, req: MovementsReq) -> Result<Movements> {
        Ok(self
            .client()
            .await?
            .get_movements(tonic::Request::new(req.into()))
            .await?
            .into_inner()
            .movements
            .into_iter()
            .map(|m| m.into())
            .collect())
    }
    pub async fn correlating_tickers(&self, req: CorrelatingTickersReq) -> Result<ActixStream> {
        let stream = self
            .client()
            .await?
            .get_correlating_tickers(tonic::Request::new(req.into()))
            .await?
            .into_inner();

        let to_json = |t: db_proto::Correl| -> Result<String> {
            let t: CorrelatingTickers = t.into();
            let js = serde_json::to_string(&t).unwrap();
            Ok(js)
        };

        Ok(gprc_to_stream(stream, to_json).await)
    }
    pub async fn mutual_correlations(
        &self,
        tickers: &Vec<BasicTicker>,
        until: Option<&str>,
        period: u32,
    ) -> Result<Vec<MutualCorrel>> {
        let mut client = self.client().await?;
        let mutual_correls = client
            .get_mutual_correlations(tonic::Request::new(db_proto::CorrelReq {
                tickers: tickers
                    .into_iter()
                    .map(|t: &BasicTicker| t.clone().into())
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
    pub async fn security_data(&self, req: TimeSeriesReq) -> Result<ActixStream> {
        let stream = self
            .client()
            .await?
            .get_security_data(tonic::Request::new(req.into()))
            .await?
            .into_inner();

        let to_json = |t: db_proto::TimeSeriesData| -> Result<String> {
            let t: TimeSeriesData = t.into();
            let js = serde_json::to_string(&t).unwrap();
            Ok(js)
        };
        Ok(gprc_to_stream(stream, to_json).await)
    }
    pub async fn portfolio(&self, portfolio_id: String) -> Result<Portfolio> {
        let mut client = self.client().await?;
        Ok(client
            .get_portfolio(tonic::Request::new(db_proto::Id { id: portfolio_id }))
            .await?
            .into_inner()
            .into())
    }
    pub async fn portfolios(&self, filter: String) -> Result<Portfolios> {
        let mut client = self.client().await?;
        Ok(client
            .get_portfolios(tonic::Request::new(db_proto::PortfolioReq {
                filter: filter.to_string(),
            }))
            .await?
            .into_inner()
            .into())
    }
    pub async fn portfolio_securities(&self, portfolio_id: String) -> Result<PortfolioSecurities> {
        let mut client = self.client().await?;
        Ok(client
            .get_portfolio_securities(tonic::Request::new(db_proto::Id { id: portfolio_id }))
            .await?
            .into_inner()
            .securities
            .into_iter()
            .map(|s| s.into())
            .collect())
    }
    pub async fn portfolio_profits(&self, req: SecurityProfitReq) -> Result<SecurityProfits> {
        let mut client = self.client().await?;
        Ok(client
            .get_portfolio_profits(tonic::Request::new(req.into()))
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
