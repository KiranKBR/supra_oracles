use crate::counter::Counter;
use crate::file_operation::save_to_file;
use crate::key_counter::{verify_signature, KeyCounter};
use crate::ws_process::listen_to_websocket;

pub async fn cache_mode(duration_seconds: u64) {
    //--------------------------included WS_URLS and Related data------------------------------------------------------
    let binance_url = "wss://stream.binance.com:9443/ws/btcusdt@trade"; //1
                                                                        // let binace_key =
    let gemini_url = "wss://api.gemini.com/v1/marketdata/BTCUSD"; //2
    let bybit_url = "wss://stream.bybit.com/realtime"; //3
    let bybit_msg = r#"{"op": "subscribe", "args": ["trade.BTCUSD"]}"#;
    let krakel_url = "wss://ws.kraken.com/"; //4
    let kraken_msg =
        r#"{"event":"subscribe", "subscription":{"name":"ticker"}, "pair":["BTC/USD"]}"#;

    let bitfinex_url = "wss://api-pub.bitfinex.com/ws/2"; //5
    let bitfinex_msg = r#"{
        "event": "subscribe",
        "channel": "ticker",
        "symbol": "tBTCUSD"
    }"#;
    //----------------------------------------------------------------------------------------------------------------------

    let res_counter = std::sync::Arc::new(std::sync::Mutex::new(Counter::new())); //for file operations
    let key_counter = std::sync::Arc::new(std::sync::Mutex::new(KeyCounter::new())); //for signatures

    //--------------------------------tasks--------------------------------------------------------------------------------
    let gemini_task = tokio::spawn(listen_to_websocket(
        "Gemini".into(),
        gemini_url,
        duration_seconds,
        None,
        res_counter.clone(),
        key_counter.clone(),
    ));
    let bybit_task = tokio::spawn(listen_to_websocket(
        "Bybit".into(),
        bybit_url,
        duration_seconds,
        Some(bybit_msg),
        res_counter.clone(),
        key_counter.clone(),
    ));
    let binanche_task = tokio::spawn(listen_to_websocket(
        "Binance".into(),
        binance_url,
        duration_seconds,
        None,
        res_counter.clone(),
        key_counter.clone(),
    ));
    let kraken_task = tokio::spawn(listen_to_websocket(
        "Kraken".into(),
        krakel_url,
        duration_seconds,
        Some(kraken_msg),
        res_counter.clone(),
        key_counter.clone(),
    ));
    let bitfinex_task = tokio::spawn(listen_to_websocket(
        "Bitfinex".into(),
        bitfinex_url,
        duration_seconds,
        Some(bitfinex_msg),
        res_counter.clone(),
        key_counter.clone(),
    ));
    //------------------------------------------------------------------------------------------------------------

    let res = tokio::try_join!(
        gemini_task,
        bybit_task,
        binanche_task,
        kraken_task,
        bitfinex_task
    )
    .unwrap();

    let mut sum = 0.0;
    let mut count = 0;
    for result in [res.0, res.1, res.2, res.3, res.4] {
        match result {
            Ok(pair) => {
                let verify = verify_signature(
                    key_counter
                        .lock()
                        .unwrap()
                        .public_keys
                        .get(&pair.0)
                        .unwrap(),
                    pair.1.to_string().as_str(),
                    &pair.2,
                ); //verifying the signature-------------------
                match verify {
                    true => {
                        if pair.1 != 0.0 {
                            sum += pair.1;
                            count += 1;
                        }
                    }
                    false => eprintln!("Signature verification failed"),
                }
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
            }
        }
    }
    let average = sum / count as f64;
    //format to store the data in btcusd_average.txt--------------------------this is the aggregated result
    let content = format!(
        "The Average USD price of BTC is: {} \n {:?}",
        average,
        res_counter.lock().unwrap().data
    );
    //---------------------------------------------------------------------------------------------------
    let file_path = format!("btcusd_average.txt");
    let _ = save_to_file(file_path.as_str(), &content).await;
    println!(
        "Total Cache complete. The average USD price of BTC is: {}",
        average
    );
}
