use crate::counter::Counter;
use crate::file_operation::save_to_file;
use crate::key_counter::{generate_key_pair, sign_message, KeyCounter};
use futures_util::stream::SplitStream;
use futures_util::SinkExt;
use futures_util::StreamExt;
use serde_json::Value;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::error::Error as WsError;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
//-----------------------------------------------------------------------------------------
pub async fn listen_to_websocket(
    exchange: String,
    ws_url: &str,
    duration_seconds: u64,
    subscribe_msg: Option<&str>,
    res_counter: std::sync::Arc<std::sync::Mutex<Counter>>,
    key_counter: std::sync::Arc<std::sync::Mutex<KeyCounter>>,
) -> Result<(String, f64, Vec<u8>), WsError> {
    //
    let ws_stream = connect_async(ws_url)
        .await
        .expect("Error connecting to WebSocket")
        .0;

    let (mut write, read) = ws_stream.split();
    let counter = std::sync::Arc::new(std::sync::Mutex::new(Counter::new()));

    if let Some(msg) = subscribe_msg {
        write.send(Message::Text(msg.to_string())).await?;
    }

    tokio::spawn(read_messages(exchange.clone(), read, counter.clone()));
    sleep(Duration::from_secs(duration_seconds)).await;
    write.send(Message::Close(None)).await?;
    //

    let average = counter.lock().unwrap().calculate_average();
    let data_str: String = counter.lock().unwrap().data.join("\n");
    let content = format!("average {}\nData points: \n{}", average, data_str);
    let file_path = format!("result_data/{}_result_and_data_points.txt", exchange); //storing single socket result and data points in directory result_data
    save_to_file(file_path.as_str(), &content).await?;
    //

    println!(
        "Cache complete. The average USD price of BTC is: {}",
        average
    );
    if average != 0.0 {
        res_counter.lock().unwrap().add_price(average, data_str);
    }

    //

    let (private_key, public_key) = generate_key_pair(); //generating key pair
    let signed_message = sign_message(&private_key, average.to_string().as_str()); //signing message
    key_counter
        .lock()
        .unwrap()
        .public_keys
        .insert(exchange.clone(), public_key);
    //
    Ok((exchange, average, signed_message))
}

pub async fn read_messages(
    exchange: String,
    mut read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    counter: std::sync::Arc<std::sync::Mutex<Counter>>,
) {
    while let Some(Ok(message)) = read.next().await {
        match message {
            Message::Text(text) => {
                let json_value: Value = serde_json::from_str(&text).unwrap();
                if exchange.eq("Gemini".into()) {
                    let price = json_value
                        .get("events")
                        .and_then(|events| events.get(0))
                        .and_then(|event| event.get("price"))
                        .and_then(|price| price.as_str())
                        .and_then(|price_str| price_str.parse::<f64>().ok())
                        .unwrap();
                    counter.lock().unwrap().add_price(price, text);
                } else if exchange.eq("Bybit".into()) {
                    if let Some(data) = json_value.pointer("/data/0/price") {
                        let price = data.as_f64().unwrap();
                        counter.lock().unwrap().add_price(price, text);
                    }
                } else if exchange.eq("Binance".into()) {
                    let price = json_value
                        .get("p")
                        .and_then(|price| price.as_str())
                        .and_then(|prce_str| prce_str.parse::<f64>().ok())
                        .unwrap();
                    counter.lock().unwrap().add_price(price, text);
                } else if exchange.eq("Kraken".into()) {
                    if let Some(data) = json_value.pointer("/1/p") {
                        let price = data
                            .get(0)
                            .and_then(|price_str| price_str.as_str())
                            .and_then(|price| price.parse::<f64>().ok())
                            .unwrap();
                        counter.lock().unwrap().add_price(price, text);
                    }
                } else if exchange.eq("Bitfinex".into()) {
                    if let Some(data) = json_value.pointer("/1/6") {
                        counter
                            .lock()
                            .unwrap()
                            .add_price(data.as_f64().unwrap(), text);
                    }
                }
            }
            _ => {}
        }
    }
}
